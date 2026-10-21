import { createPublicKey, verify } from 'node:crypto';
import { PublicKey, TransactionMessage, TransactionInstruction, ComputeBudgetProgram } from '@solana/web3.js';
import { assert, stable } from './proof.mjs';

const TOKEN = new PublicKey('TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA');
const ASSOCIATED_TOKEN = new PublicKey('ATokenGPvbdGVxr1b2hvZbsiqW5xWH25efTNsLJA8knL');
const MINT = new PublicKey('EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v');
const MEMO = new PublicKey('MemoSq4gqABAXKb96qnH8TysNcWxMyWCqXgDLGmfcHr');
const associated = owner => PublicKey.findProgramAddressSync([owner.toBuffer(), TOKEN.toBuffer(), MINT.toBuffer()], ASSOCIATED_TOKEN)[0];

function expected(quote, payer, blockhash, units) {
  const buyer = new PublicKey(payer), destination = new PublicKey(quote.payTo);
  const transfer = Buffer.alloc(10); transfer[0] = 12; transfer.writeBigUInt64LE(BigInt(quote.amount), 1); transfer[9] = 6;
  return new TransactionMessage({ payerKey: new PublicKey(quote.extra.feePayer), recentBlockhash: blockhash,
    instructions: [ComputeBudgetProgram.setComputeUnitLimit({ units }), ComputeBudgetProgram.setComputeUnitPrice({ microLamports: 1 }),
      new TransactionInstruction({ programId: TOKEN, data: transfer, keys: [
        { pubkey: associated(buyer), isSigner: false, isWritable: true },
        { pubkey: MINT, isSigner: false, isWritable: false },
        { pubkey: associated(destination), isSigner: false, isWritable: true },
        { pubkey: buyer, isSigner: true, isWritable: false },
      ] }), new TransactionInstruction({ programId: MEMO, data: Buffer.from(quote.extra.memo), keys: [] })],
  }).compileToV0Message();
}

function shape(message) {
  assert(message.addressTableLookups.length === 0, 'payment_lookup_tables_forbidden');
  const keys = message.staticAccountKeys.map(key => key.toBase58()), h = message.header;
  assert(new Set(keys).size === keys.length, 'payment_duplicate_accounts');
  const roles = keys.map((key, i) => [key, i < h.numRequiredSignatures,
    i < h.numRequiredSignatures ? i < h.numRequiredSignatures - h.numReadonlySignedAccounts : i < keys.length - h.numReadonlyUnsignedAccounts]);
  const instructions = message.compiledInstructions.map(ix => {
    assert(keys[ix.programIdIndex] && ix.accountKeyIndexes.every(i => roles[i]), 'payment_invalid_account_index');
    return [keys[ix.programIdIndex], Array.from(ix.accountKeyIndexes, i => roles[i]), Buffer.from(ix.data).toString('base64')];
  });
  // Solana Kit and solders order readonly account tables differently. Compare
  // resolved instruction addresses/permissions/data without rewriting the wire.
  return stable([keys[0], keys[1], [...roles].sort((a, b) => a[0] < b[0] ? -1 : a[0] > b[0] ? 1 : 0), instructions]);
}

export function validatePaymentTransaction(tx, quote, payer) {
  assert(tx.message.version === 0, 'payment_must_be_v0');
  assert(tx.signatures.length === 2 && tx.signatures[0].every(byte => byte === 0), 'payment_fee_payer_must_be_unsigned');
  const actual = shape(tx.message);
  // Only a 118-byte v2 memo needs more than the official client's 20k limit.
  const allowed = quote.extra.memo.startsWith('covenant.x402.v2:') ? [20_000, 60_000] : [20_000];
  assert(allowed.some(units => actual === shape(expected(quote, payer, tx.message.recentBlockhash, units))), 'payment_transaction_not_approved');
  const publicKey = createPublicKey({ key: Buffer.concat([Buffer.from('302a300506032b6570032100', 'hex'), new PublicKey(payer).toBuffer()]), format: 'der', type: 'spki' });
  assert(verify(null, Buffer.from(tx.message.serialize()), publicKey, tx.signatures[1]), 'invalid_payment_signature');
}
