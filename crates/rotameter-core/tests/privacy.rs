#![forbid(unsafe_code)]
//! Integration tests for `HMAC-SHA256` visitor hashing and UTC salt rotation windows.

use rotameter_core::id::SiteId;
use rotameter_core::privacy::{
    GRACE_WINDOW_MS, InMemorySaltProvider, MS_PER_DAY, SaltProvider, SaltWindow,
    compute_api_key_hash, compute_visitor_id, resolve_salt_window, utc_hour_floor_ms, utc_yyyymmdd,
};
use rotameter_core::types::SaltRotation;

#[test]
fn sha256_api_key_and_hmac_visitor_hashing_are_deterministic_and_isolated() {
    let abc_hash = compute_api_key_hash("abc");
    assert_eq!(abc_hash[0], 0xba);
    assert_eq!(abc_hash[1], 0x78);
    assert_eq!(abc_hash[31], 0xad);

    let site_a = SiteId::from_uuidv7_parts(1_700_000_000_000, [1; 10]);
    let site_b = SiteId::from_uuidv7_parts(1_700_000_000_000, [2; 10]);
    let salt_1 = [0x11; 32];
    let salt_2 = [0x22; 32];

    let v1 = compute_visitor_id(&salt_1, site_a, "198.51.100.4", "Mozilla/5.0");
    assert_eq!(
        v1,
        compute_visitor_id(&salt_1, site_a, "198.51.100.4", "Mozilla/5.0")
    );
    assert_ne!(
        v1,
        compute_visitor_id(&salt_1, site_b, "198.51.100.4", "Mozilla/5.0")
    );
    assert_ne!(
        v1,
        compute_visitor_id(&salt_1, site_a, "198.51.100.5", "Mozilla/5.0")
    );
    assert_ne!(
        v1,
        compute_visitor_id(&salt_2, site_a, "198.51.100.4", "Mozilla/5.0")
    );

    let ua_long_a = format!("{}X", "A".repeat(512));
    let ua_long_b = format!("{}Y", "A".repeat(512));
    assert_eq!(
        compute_visitor_id(&salt_1, site_a, "198.51.100.4", &ua_long_a),
        compute_visitor_id(&salt_1, site_a, "198.51.100.4", &ua_long_b)
    );
}

#[test]
fn salt_windows_cover_daily_weekly_monthly_and_grace_boundary() {
    let feb_29_2355 = 19_782 * MS_PER_DAY + (23 * 60 + 55) * 60_000;
    let mar_01_0005 = 19_783 * MS_PER_DAY + 5 * 60_000;
    let mar_01_0030 = 19_783 * MS_PER_DAY + GRACE_WINDOW_MS;

    assert_eq!(utc_yyyymmdd(feb_29_2355), 20_240_229);
    assert_eq!(utc_yyyymmdd(mar_01_0005), 20_240_301);
    assert_eq!(
        utc_hour_floor_ms(feb_29_2355),
        19_782 * MS_PER_DAY + 23 * 3_600_000
    );

    let daily_before = resolve_salt_window(SaltRotation::Daily, feb_29_2355);
    assert!(daily_before.previous_in_grace.is_none());

    let daily_grace = resolve_salt_window(SaltRotation::Daily, mar_01_0005);
    assert_eq!(
        daily_grace.previous_in_grace.unwrap().window_id,
        daily_before.current.window_id
    );

    let daily_after = resolve_salt_window(SaltRotation::Daily, mar_01_0030);
    assert!(daily_after.previous_in_grace.is_none());

    let monthly_feb = resolve_salt_window(SaltRotation::Monthly, feb_29_2355);
    let monthly_mar = resolve_salt_window(SaltRotation::Monthly, mar_01_0005);
    assert_eq!(
        monthly_mar.previous_in_grace.unwrap().window_id,
        monthly_feb.current.window_id
    );

    let sunday = resolve_salt_window(SaltRotation::Weekly, 20_779 * MS_PER_DAY + MS_PER_DAY - 1);
    let monday = resolve_salt_window(SaltRotation::Weekly, 20_780 * MS_PER_DAY + 60_000);
    assert_eq!(
        monday.previous_in_grace.unwrap().window_id,
        sunday.current.window_id
    );

    let provider = InMemorySaltProvider::new(|w: SaltWindow| {
        [u8::try_from(w.window_id % 200).unwrap_or(1); 32]
    });
    let w_old = SaltWindow {
        rotation: SaltRotation::Daily,
        window_id: 20_000,
        start_ms: 20_000 * MS_PER_DAY,
    };
    assert_eq!(provider.insert_salt(w_old, [9; 32]), Ok(()));
    assert_eq!(provider.salt_for(w_old), Ok([9; 32]));
    assert_eq!(provider.prune_before(SaltRotation::Daily, 20_001), Ok(()));
}
