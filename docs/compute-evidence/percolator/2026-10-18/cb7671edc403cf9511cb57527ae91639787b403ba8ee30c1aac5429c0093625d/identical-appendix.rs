
// PR #239 reduced public-route appendix: Long side, flat obligation, fixed weight.
fn pr239_cover_forfeit(ok: bool, long_residue: u8, short_residue: u8) {
    kani::cover!(
        ok && long_residue == 0 && short_residue == 0,
        "pr239 forfeit zero residue"
    );
    kani::cover!(
        ok && long_residue > 0 && short_residue == 0,
        "pr239 forfeit long-only residue"
    );
    kani::cover!(
        ok && long_residue == 0 && short_residue > 0,
        "pr239 forfeit short-only residue"
    );
    kani::cover!(
        ok && long_residue > 0 && short_residue > 0,
        "pr239 forfeit two-sided residue"
    );
}

fn pr239_cover_auto_crank(ok: bool, residue: u8) {
    kani::cover!(ok && residue == 0, "pr239 auto-crank zero residue");
    kani::cover!(ok && residue > 0, "pr239 auto-crank two-sided residue");
}

#[kani::proof]
#[kani::unwind(24)]
#[kani::solver(cadical)]
fn proof_v16_public_forfeit_long_flat_residue() {
    use percolator::v16::DeadLegForfeitOutcomeV16;

    let long_residue: u8 = kani::any();
    let short_residue: u8 = kani::any();
    kani::assume(long_residue <= 3 && short_residue <= 3);
    let weight = POS_SCALE;

    let (mut header, mut markets, mut account_header) = one_market_view_fixture();
    let market_id = markets[0].engine.asset.market_id.get();
    header.vault = V16PodU128::new(7 * POS_SCALE);
    header.c_tot = V16PodU128::new(5 * POS_SCALE);
    header.insurance = V16PodU128::new(2 * POS_SCALE);
    header.resolved_payout_blocker_count = V16PodU64::new(1);
    header.materialized_portfolio_count = V16PodU64::new(1);
    account_header.capital = V16PodU128::new(5 * POS_SCALE);
    let mut asset = markets[0].engine.asset.try_to_runtime().unwrap();
    asset.lifecycle = AssetLifecycleV16::Recovery;
    asset.a_long = 999_999_940_959_838;
    asset.a_short = 999_999_952_247_457;
    asset.oi_eff_long_q = long_residue as u128;
    asset.oi_eff_short_q = short_residue as u128;
    asset.stored_pos_count_long = 1;
    asset.pending_obligation_count_long = 1;
    asset.loss_weight_sum_long = weight;
    markets[0].engine.asset = AssetStateV16Account::from_runtime(&asset);
    account_header.legs[0] = PortfolioLegV16Account::from_runtime(&PortfolioLegV16 {
        active: true,
        asset_index: 0,
        market_id,
        side: SideV16::Long,
        basis_pos_q: 0,
        a_basis: ADL_ONE,
        k_snap: 0,
        f_snap: 0,
        kf_epoch_snap: 0,
        epoch_snap: 0,
        loss_weight: weight,
        b_snap: 0,
        b_rem: 0,
        b_epoch_snap: 0,
        b_stale: false,
        stale: false,
    });
    let mut bitmap = account_header.active_bitmap.map(V16PodU64::get);
    active_bitmap_set(&mut bitmap, 0).unwrap();
    account_header.active_bitmap = bitmap.map(V16PodU64::new);
    let risk_epoch_before = header.risk_epoch.get();
    let vault_before = header.vault;
    let c_tot_before = header.c_tot;
    let insurance_before = header.insurance;
    let capital_before = account_header.capital;
    let pnl_before = account_header.pnl;

    let mut market = MarketGroupV16ViewMut::new(&mut header, &mut markets);
    let mut account = PortfolioV16ViewMut::new(&mut account_header);
    let result = market.forfeit_recovery_leg_not_atomic(&mut account, 0, u128::MAX);
    let after = market.markets[0].engine.asset.try_to_runtime().unwrap();

    pr239_cover_forfeit(result.is_ok(), long_residue, short_residue);
    assert!(
        result
            == Ok(DeadLegForfeitOutcomeV16 {
                detached: true,
                positive_pnl_forfeited: 0,
                loss_settled: 0,
                support_consumed: 0,
                junior_face_burned: 0,
                principal_used: 0,
                insurance_used: 0,
                residual_booked: 0,
                explicit_loss: 0,
            })
            && active_bitmap_is_empty(account.header.active_bitmap.map(V16PodU64::get))
            && after.oi_eff_long_q == 0
            && after.oi_eff_short_q == 0
            && after.loss_weight_sum_long == 0
            && after.loss_weight_sum_short == 0
            && after.stored_pos_count_long == 0
            && after.stored_pos_count_short == 0
            && after.pending_obligation_count_long == 0
            && after.pending_obligation_count_short == 0
            && after.mode_long == SideModeV16::ResetPending
            && after.mode_short == SideModeV16::ResetPending
            && market.header.risk_epoch.get() == risk_epoch_before + 2
            && market.header.resolved_payout_blocker_count.get() == 0
            && market.header.vault == vault_before
            && market.header.c_tot == c_tot_before
            && market.header.insurance == insurance_before
            && account.header.capital == capital_before
            && account.header.pnl == pnl_before,
        "PR239_FORFEIT_LONG_FLAT_RELEASE_INVARIANTS"
    );
}

#[kani::proof]
#[kani::unwind(24)]
#[kani::solver(cadical)]
fn proof_v16_public_auto_crank_long_residue() {
    use percolator::v16::{AutoCrankOutcomeV16, AutoCrankResultV16, AutoCrankWorkV16};

    let residue: u8 = kani::any();
    kani::assume(residue <= 3);
    let weight = POS_SCALE;

    let (mut header, mut markets, mut account_header) = one_market_view_fixture();
    install_flat_pending_obligation(
        &mut header,
        &mut markets,
        &mut account_header,
        SideV16::Long,
        weight,
        false,
    );
    header.vault = V16PodU128::new(7 * POS_SCALE);
    header.c_tot = V16PodU128::new(5 * POS_SCALE);
    header.insurance = V16PodU128::new(2 * POS_SCALE);
    account_header.capital = V16PodU128::new(5 * POS_SCALE);
    let mut asset = markets[0].engine.asset.try_to_runtime().unwrap();
    asset.a_long = 999_999_940_959_838;
    asset.a_short = 999_999_952_247_457;
    asset.oi_eff_long_q = residue as u128;
    asset.oi_eff_short_q = residue as u128;
    markets[0].engine.asset = AssetStateV16Account::from_runtime(&asset);
    let risk_epoch_before = header.risk_epoch.get();
    let vault_before = header.vault;
    let c_tot_before = header.c_tot;
    let insurance_before = header.insurance;
    let capital_before = account_header.capital;
    let pnl_before = account_header.pnl;

    let mut market = MarketGroupV16ViewMut::new(&mut header, &mut markets);
    let mut account = PortfolioV16ViewMut::new(&mut account_header);
    let now_slot = market.header.current_slot.get();
    let result = market.permissionless_auto_crank_not_atomic(
        &mut account,
        AutoCrankWorkV16 {
            now_slot,
            observations: &[],
            resolved_close_fee_rate_per_slot: 0,
        },
    );
    let after = market.markets[0].engine.asset.try_to_runtime().unwrap();

    pr239_cover_auto_crank(result.is_ok(), residue);
    assert!(
        result
            == Ok(AutoCrankResultV16 {
                selected: AutoCrankPlanV16::RefreshAccount {
                    asset_index: Some(0)
                },
                outcome: AutoCrankOutcomeV16::Progressed(
                    PermissionlessProgressOutcomeV16::AccountCurrent
                ),
            })
            && active_bitmap_is_empty(account.header.active_bitmap.map(V16PodU64::get))
            && after.oi_eff_long_q == 0
            && after.oi_eff_short_q == 0
            && after.loss_weight_sum_long == 0
            && after.loss_weight_sum_short == 0
            && after.stored_pos_count_long == 0
            && after.stored_pos_count_short == 0
            && after.pending_obligation_count_long == 0
            && after.pending_obligation_count_short == 0
            && after.mode_long == SideModeV16::ResetPending
            && after.mode_short == SideModeV16::ResetPending
            && market.header.risk_epoch.get() == risk_epoch_before + 2
            && market.header.resolved_payout_blocker_count.get() == 0
            && market.header.vault == vault_before
            && market.header.c_tot == c_tot_before
            && market.header.insurance == insurance_before
            && account.header.capital == capital_before
            && account.header.pnl == pnl_before,
        "PR239_AUTO_CRANK_LONG_RELEASE_INVARIANTS"
    );
}
