"""Variable upfront quote regression tests; rates are unfunded fixture data."""
import pytest
from pathlib import Path

from covenant_compute_x402 import CovenantError
from covenant_compute_x402.proof import compact, strict_json, stable, sha256, verify_compute_result
from fixture import Fixture, request


def priced_request(model, max_tokens):
    return {**request(), "model": model, "max_tokens": max_tokens, "pricing": "model_tariff_v1"}


def test_real_rust_tariff_rounding_and_offline_replay_after_expiry():
    f = strict_json((Path(__file__).parent / "fixtures/native-tariff-response.json").read_bytes())
    proof = f["response"]["covenant"]
    verified = verify_compute_result(f["response"], request=f["request"], request_sha256=sha256(stable(f["request"])),
                                     quote_id=proof["quote_id"], gateway_buyer=proof["envelope"]["signer_pubkey_b58"],
                                     operators=[proof["receipt"]["signer_pubkey_b58"]])
    assert verified["job"]["price_micro_usdc"] == 165
    assert verified["job"]["capability_requirement"]["inference_tariff"]["requested_output_cap"] == 64


@pytest.mark.parametrize("mutation,error", [
    ("downgrade", "inference_tariff_required"),
    ("routing", "inference_tariff_requirement_mismatch"),
    ("rounding", "inference_tariff_price_mismatch"),
    ("model", "inference_tariff_model_mismatch"),
    ("cap", "inference_tariff_cap_mismatch"),
    ("revision", "invalid_inference_tariff"),
    ("time", "inference_tariff_issue_time_mismatch"),
    ("unknown", "invalid_inference_tariff_quote"),
])
async def test_signed_tariff_mutations_keep_payment_reserved(tmp_path, mutation, error):
    def change(job):
        quote = job["input"][2]["value"]["inference_tariff"]
        if mutation == "downgrade":
            job["input"].pop()
            del job["capability_requirement"]["inference_tariff"]
            return
        if mutation == "routing":
            job["capability_requirement"]["inference_tariff"]["tariff"]["revision"] += 1
            return
        if mutation == "rounding":
            quote["price_micro_usdc"] -= 1
            job["price_micro_usdc"] = quote["price_micro_usdc"]
        elif mutation == "model":
            quote["tariff"]["model"] = "another-model"
        elif mutation == "cap":
            quote["requested_output_cap"] += 1
        elif mutation == "revision":
            quote["tariff"]["revision"] = 0
        elif mutation == "time":
            job["issued_at_ms"] = quote["tariff"]["valid_until_ms"]
        elif mutation == "unknown":
            quote["actual_tokens"] = 1
        job["capability_requirement"]["inference_tariff"] = strict_json(compact(quote))
    f, item = Fixture(tmp_path, bad_job=change), priced_request("small-model", 100)
    async with f.client() as client:
        with pytest.raises(CovenantError, match=error):
            await client.purchase(**item)
        saved = client.journal.get(item["request_id"])
        assert saved["payment"] and not saved["result"] and saved["amount"] == 1000


async def test_model_and_requested_output_cap_share_durable_ceiling(tmp_path):
    def price(body):
        return (1000 + body["max_tokens"] if body["model"] == "small-model"
                else 2000 + 2 * body["max_tokens"])
    f = Fixture(tmp_path, price=price)
    caps = dict(max_payment_micro_usdc=4000, max_total_micro_usdc=5000)
    async with f.client(**caps) as client:
        assert (await client.purchase(**priced_request("small-model", 100)))["amountMicroUsdc"] == 1100
        assert (await client.purchase(**priced_request("large-model", 100)))["amountMicroUsdc"] == 2200
    async with f.client(**caps) as client:
        assert (await client.purchase(**priced_request("small-model", 600)))["amountMicroUsdc"] == 1600
        with pytest.raises(CovenantError, match="cumulative_spend_cap_exceeded"):
            await client.purchase(**priced_request("small-model", 1))
        with pytest.raises(CovenantError, match="per_payment_cap_exceeded"):
            await client.purchase(**priced_request("large-model", 1001))
    assert f.wallet.signatures == f.sends == 3


async def test_pending_replay_preserves_original_quote_across_tariff_change(tmp_path):
    base = 1000
    f = Fixture(tmp_path, price=lambda body: base + body["max_tokens"], first_status=202)
    caps = dict(max_payment_micro_usdc=10_000, max_total_micro_usdc=20_000)
    item = priced_request("small-model", 100)
    async with f.client(**caps) as client:
        assert (await client.purchase(**item))["status"] == "pending"
        original = client.journal.get(item["request_id"])
    base = 8000
    async with f.client(**caps) as client:
        for change in [{"model": "large-model"}, {"max_tokens": 101}, {"pricing": None}]:
            with pytest.raises(CovenantError, match="idempotency_request_mismatch"):
                await client.purchase(**{**item, **change})
        result = await client.purchase(**item)
        assert result["amountMicroUsdc"] == 1100
        saved = client.journal.get(item["request_id"])
        for field in ["quote", "payment", "amount"]:
            assert saved[field] == original[field]
        assert f.quotes == f.wallet.signatures == 1
        assert f.payments[0] == f.payments[1]
        calls = len(f.calls)
        assert await client.purchase(**item) == result
        assert len(f.calls) == calls
        assert (await client.purchase(**priced_request("small-model", 100)))["amountMicroUsdc"] == 8100


async def test_changed_reserved_amount_refuses_dispatch(tmp_path):
    f, item = Fixture(tmp_path, first_status=202), request()
    async with f.client() as client:
        await client.purchase(**item)
        client.journal.db.execute("UPDATE purchases SET amount=? WHERE id=?", (999, item["request_id"]))
        with pytest.raises(CovenantError, match="reserved_amount_mismatch"):
            await client.purchase(**item)
    assert f.sends == f.wallet.signatures == 1


async def test_completed_replay_refuses_changed_amount_offline(tmp_path):
    f, item = Fixture(tmp_path), request()
    async with f.client() as client:
        result = await client.purchase(**item)
        result["amountMicroUsdc"] = 999
        client.journal.db.execute("UPDATE purchases SET result=? WHERE id=?", (compact(result), item["request_id"]))
        calls = len(f.calls)
        with pytest.raises(CovenantError, match="cached_amount_mismatch"):
            await client.purchase(**item)
        assert len(f.calls) == calls


async def test_legacy_request_refuses_unexpected_signed_tariff(tmp_path):
    f = Fixture(tmp_path, bad_job=lambda job: job["input"].append({"type": "json", "value": {"inference_tariff": {}}}))
    async with f.client() as client:
        with pytest.raises(CovenantError, match="unexpected_inference_tariff"):
            await client.purchase(**request())
