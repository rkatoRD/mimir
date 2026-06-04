use nr_core::Bits;

const TBS_TABLE: [u32; 93] = [
    24, 32, 40, 48, 56, 64, 72, 80, 88, 96, 104, 112, 120, 128, 136, 144, 152, 160, 168, 176, 184, 192, 208, 224, 240, 256, 272, 288, 304, 320, 336, 352, 368, 384, 408, 432, 456, 480, 504, 528, 552, 576, 608, 640, 672, 704, 736, 768, 808, 848, 888, 928, 984, 1032, 1064, 1128, 1160, 1192, 1224, 1256, 1288, 1320, 1352, 1416, 1480, 1544, 1608, 1672, 1736, 1800, 1864, 1928, 2024, 2088, 2152, 2216, 2280, 2408, 2472, 2536, 2600, 2664, 2728, 2792, 2856, 2976, 3104, 3240, 3368, 3496, 3624, 3752, 3824,
];

fn tbs_from_table(n_info_quant: u32) -> u32 {
    for &t in TBS_TABLE.iter() {
        if t >= n_info_quant {
            return t;
        }
    }
    *TBS_TABLE.last().unwrap()
}

pub fn compute_tbs(
    n_re_per_rb: u32,
    n_prb: u32,
    code_rate: f64,
    modulation_order: u8,
    num_layers: u8,
) -> Bits {
    let n_re = n_re_per_rb.min(156) * n_prb;
    let n_info = n_re as f64 * code_rate * modulation_order as f64 * num_layers as f64;

    let bits = if n_info <= 3824.0 {
        let n = (n_info.floor() as i64 - 24).max(0) as f64;
        let exp = (n.log2().floor() as i32 - 5).max(0);
        let scale = 2f64.powi(exp);
        let n_info_quant = (scale * (n_info / scale).round()).max(24.0) as u32;

        tbs_from_table(n_info_quant)
    } else {
        let exp = (n_info.log2().floor() as i32 - 5).max(0);
        let scale = 2f64.powi(exp);
        let n_info_quant = (scale * ((n_info - 24.0) / scale).round()).max(3840.0);

        if code_rate <= 0.25 {
            let c = ((n_info_quant + 24.0) / 3816.0).ceil();
            (8.0 * c * ((n_info_quant + 24.0) / (8.0 * c)).ceil() - 24.0) as u32
        } else if n_info_quant > 8424.0 {
            let c = ((n_info_quant + 24.0) / 8424.0).ceil();
            (8.0 * c * ((n_info_quant + 24.0) / (8.0 * c)).ceil() - 24.0) as u32
        } else {
            (8.0 * ((n_info_quant + 24.0) / 8.0).ceil() - 24.0) as u32
        }
    };

    Bits::new(bits as u64)
}