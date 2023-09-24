const POLYNOMIAL: u32 = 0x04C11DB7;

pub fn digest(bytes: &[u8]) -> u32 {
    let mut crc = !0;
    for byte in bytes {
        crc = crc ^ ((byte.reverse_bits() as u32) << 24);
        for _ in 0..8 {
            if crc & (1 << 31) > 0 {
                crc = (crc << 1) ^ POLYNOMIAL;
            } else {
                crc = crc << 1;
            }
        }
    }
    !crc.reverse_bits()
}
