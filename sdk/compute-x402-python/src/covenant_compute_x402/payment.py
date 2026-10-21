"""Official x402 SVM construction, unsigned inspection, then bounded signing."""
from __future__ import annotations

import base64
import inspect
from types import SimpleNamespace

from solders.hash import Hash
from solders.instruction import AccountMeta, Instruction
from solders.keypair import Keypair
from solders.message import MessageV0, to_bytes_versioned
from solders.pubkey import Pubkey
from solders.signature import Signature
from solders.transaction import VersionedTransaction
from x402 import x402Client
from x402.mechanisms.svm.exact import ExactSvmScheme
from x402.mechanisms.svm.utils import derive_ata
from x402.schemas import PaymentRequired

from .proof import CovenantError, require, sha256, stable

SOLANA_GENESIS = "5eykt4UsFv8P8NJdTREpY1vzqKqZKvdpKuc147dw2N9d"
NETWORK = "solana:5eykt4UsFv8P8NJdTREpY1vzqKqZKvdp"
USDC = "EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v"
TOKEN_PROGRAM = "TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA"
COMPUTE_PROGRAM = "ComputeBudget111111111111111111111111111111"
MEMO_PROGRAM = "MemoSq4gqABAXKb96qnH8TysNcWxMyWCqXgDLGmfcHr"
# The official client's limit. A v3 memo keeps the whole payment near 13k CU,
# so the transaction is signed exactly as the official client built it.
PAYMENT_COMPUTE_UNITS = 20_000
# A v2 memo (118 bytes) needs 43,642 CU. Quotes from a gateway that still
# issues them are signed at this limit.
LEGACY_PAYMENT_COMPUTE_UNITS = 60_000


def short_memo(quote_id: str, request_sha256: str) -> str:
    """First 16 bytes of SHA-256 over the v3 binding, as 32 hex characters."""
    return sha256(f"covenant.x402.v3:{quote_id}:{request_sha256}")[:32]


def solana_quote(required: dict) -> dict:
    """The quote's Solana mainnet offer; offers on other networks (USDC on Base) are ignored."""
    require(isinstance(required.get("accepts"), list), "ambiguous_payment_requirements")
    offers = [quote for quote in required["accepts"] if isinstance(quote, dict) and quote.get("network") == NETWORK]
    require(offers, "untrusted_payment_rail")
    require(len(offers) == 1, "ambiguous_payment_requirements")
    return offers[0]


def payment_units(quote: dict) -> int:
    """Compute-unit limit for the quote's memo form; refuses a memo that doesn't match the quote."""
    extra = quote.get("extra") if isinstance(quote, dict) else None
    require(isinstance(extra, dict) and all(isinstance(extra.get(key), str) for key in ("memo", "quoteId", "requestSha256")),
            "quote_memo_mismatch")
    if extra["memo"] == f"covenant.x402.v2:{extra['quoteId']}:{extra['requestSha256']}":
        return LEGACY_PAYMENT_COMPUTE_UNITS
    require(extra["memo"] == short_memo(extra["quoteId"], extra["requestSha256"]), "quote_memo_mismatch")
    return PAYMENT_COMPUTE_UNITS


def decode64(value: str) -> bytes:
    try:
        require(isinstance(value, str), "invalid_base64")
        return base64.b64decode(value, validate=True)
    except (ValueError, TypeError) as exc:
        raise CovenantError("invalid_base64") from exc


def wallet_address(wallet) -> str:
    value = str(wallet.pubkey()) if isinstance(wallet, Keypair) else wallet.address
    Pubkey.from_string(value)
    return value


def expected_message(quote: dict, payer: str, blockhash: Hash, units: int) -> MessageV0:
    mint, token = Pubkey.from_string(USDC), Pubkey.from_string(TOKEN_PROGRAM)
    source = Pubkey.from_string(derive_ata(payer, USDC, TOKEN_PROGRAM))
    destination = Pubkey.from_string(derive_ata(quote["payTo"], USDC, TOKEN_PROGRAM))
    instructions = [
        Instruction(Pubkey.from_string(COMPUTE_PROGRAM), b"\x02" + units.to_bytes(4, "little"), []),
        Instruction(Pubkey.from_string(COMPUTE_PROGRAM), b"\x03" + (1).to_bytes(8, "little"), []),
        Instruction(token, b"\x0c" + int(quote["amount"]).to_bytes(8, "little") + b"\x06", [
            AccountMeta(source, False, True), AccountMeta(mint, False, False),
            AccountMeta(destination, False, True), AccountMeta(Pubkey.from_string(payer), True, False)]),
        Instruction(Pubkey.from_string(MEMO_PROGRAM), quote["extra"]["memo"].encode("utf-8"), []),
    ]
    return MessageV0.try_compile(Pubkey.from_string(quote["extra"]["feePayer"]), instructions, [], blockhash)


def validate_transaction(tx: VersionedTransaction, quote: dict, payer: str, *, unsigned: bool = False,
                         legacy_allowed: bool = False) -> None:
    require(isinstance(tx.message, MessageV0), "payment_must_be_v0")
    require(len(tx.signatures) == 2 and tx.signatures[0] == Signature.default(), "payment_fee_payer_must_be_unsigned")
    allowed_units = (PAYMENT_COMPUTE_UNITS, LEGACY_PAYMENT_COMPUTE_UNITS) if legacy_allowed else (PAYMENT_COMPUTE_UNITS,) if unsigned else (payment_units(quote),)
    def shape(message: MessageV0):
        # Kit and solders order the readonly account table differently. Compare
        # the exact instruction order, resolved addresses, permissions and data,
        # while preserving the original wire bytes for signing and settlement.
        require(not message.address_table_lookups, "payment_lookup_tables_forbidden")
        keys, h = list(message.account_keys), message.header
        require(len(keys) == len(set(map(str, keys))), "payment_duplicate_accounts")
        roles = [(str(key), i < h.num_required_signatures,
                  i < h.num_required_signatures - h.num_readonly_signed_accounts if i < h.num_required_signatures
                  else i < len(keys) - h.num_readonly_unsigned_accounts) for i, key in enumerate(keys)]
        try:
            instructions = [(str(keys[ix.program_id_index]), [roles[i] for i in ix.accounts], bytes(ix.data)) for ix in message.instructions]
        except IndexError as exc:
            raise CovenantError("payment_invalid_account_index") from exc
        return (str(keys[0]), str(keys[1]), sorted(roles), instructions)
    require(any(shape(tx.message) == shape(expected_message(quote, payer, tx.message.recent_blockhash, units)) for units in allowed_units), "payment_transaction_not_approved")
    if unsigned:
        require(all(s == Signature.default() for s in tx.signatures), "payment_must_be_unsigned")
    else:
        require(tx.signatures[1].verify(Pubkey.from_string(payer), to_bytes_versioned(tx.message)), "invalid_payment_signature")


class _UnsignedSigner:
    # The upstream Python scheme calls keypair.sign_message directly. Supplying
    # this capture signer prevents any real signature before wire inspection.
    def __init__(self, address: str):
        self.address = address
        self.keypair = self

    def sign_message(self, _message: bytes) -> Signature:
        return Signature.default()


class _SnapshotScheme(ExactSvmScheme):
    def __init__(self, payer: str, mint_data: bytes, blockhash: str):
        super().__init__(_UnsignedSigner(payer))
        self.snapshot = SimpleNamespace(
            get_account_info=lambda _mint: SimpleNamespace(value=SimpleNamespace(owner=Pubkey.from_string(TOKEN_PROGRAM), data=mint_data)),
            get_latest_blockhash=lambda: SimpleNamespace(value=SimpleNamespace(blockhash=Hash.from_string(blockhash))))

    def _get_client(self, network: str):
        require(network == NETWORK, "untrusted_payment_rail")
        return self.snapshot


async def create_payment(required: dict, wallet, payer: str, mint_data: bytes, blockhash: str) -> dict:
    client = x402Client().register(NETWORK, _SnapshotScheme(payer, mint_data, blockhash))
    quote = solana_quote(required)
    payload_model = await client.create_payment_payload(PaymentRequired.model_validate({**required, "accepts": [quote]}))
    payload = payload_model.model_dump(by_alias=True, exclude_none=True)
    require(payload.get("x402Version") == 2 and stable(payload.get("accepted")) == stable(quote), "sdk_changed_payment_requirements")
    tx = VersionedTransaction.from_bytes(decode64(payload["payload"]["transaction"]))
    validate_transaction(tx, quote, payer, unsigned=True)
    # Sign the inspected message as built; for a v2 memo, recompile it with
    # only the compute limit raised.
    units = payment_units(quote)
    message = tx.message if units == PAYMENT_COMPUTE_UNITS else expected_message(quote, payer, tx.message.recent_blockhash, units)
    unsigned = VersionedTransaction.populate(message, [Signature.default(), Signature.default()])
    before = to_bytes_versioned(message)
    if isinstance(wallet, Keypair):
        signed = VersionedTransaction.populate(message, [Signature.default(), wallet.sign_message(before)])
    else:
        signed = wallet.sign_transaction(unsigned)
        if inspect.isawaitable(signed):
            signed = await signed
    require(isinstance(signed, VersionedTransaction) and to_bytes_versioned(signed.message) == before, "wallet_changed_payment_transaction")
    validate_transaction(signed, quote, payer)
    payload["payload"]["transaction"] = base64.b64encode(bytes(signed)).decode("ascii")
    return payload
