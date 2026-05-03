#[derive(Debug, Default)]
pub struct MostroStats {
    pub successful_orders: usize,
    pub total_volume_sats: u64,
    pub first_dev_fee_ts: Option<i64>,
    pub first_order_ts: i64,
    pub last_order_ts: i64,
    pub trade_amounts: Vec<u64>,
    pub successful_trade_timestamps: Vec<i64>,
}
