use lab08::checksum::{check, checksum};

static TEST_DATA: [u8; 9] = [0x12, 0x23, 0x34, 0x45, 0x56, 0x67, 0x78, 0x89, 0x90];

#[test]
fn test_empty_data() {
    let data: Vec<u8> = vec![];
    let sum = checksum(&data);
    assert!(sum == 0xFFFF, "Checksum of empty data should be 0xFFFF");
    assert!(check(&data, sum), "Should work with empty data");
}

#[test]
fn test_valid_data_even_length() {
    let data = &TEST_DATA[..8];
    let sum = checksum(&data);
    assert!(check(&data, sum), "Valid checksum should return true");
}

#[test]
fn test_valid_data_odd_length() {
    let data = &TEST_DATA[..9];
    let sum = checksum(&data);
    assert!(
        check(&data, sum),
        "Should handle odd number of bytes correctly"
    );
}

#[test]
fn test_data_corruption() {
    let mut data = TEST_DATA;
    let sum = checksum(&data);

    data[0] ^= 0x01;

    assert!(!check(&data, sum), "Should fail if data was modified");
}

#[test]
fn test_wrong_checksum() {
    let data = TEST_DATA;
    let correct_sum = checksum(&data);
    let wrong_sum = correct_sum.wrapping_add(1);

    assert!(
        !check(&data, wrong_sum),
        "Should fail if the checksum is wrong"
    );
}

#[test]
fn test_large_data_range() {
    let data = vec![0xAB; 10_000_000];
    let sum = checksum(&data);
    assert!(check(&data, sum), "Should work with large data arrays");
}
