use candid::Principal;
use ic_websocket_cdk::ClientKey;
use lazy_static::lazy_static;

lazy_static! {
    pub static ref CLIENT_1: Principal =
        Principal::from_text("pmisz-prtlk-b6oe6-bj4fl-6l5fy-h7c2h-so6i7-jiz2h-bgto7-piqfr-7ae")
            .unwrap();
    pub static ref CLIENT_2: Principal =
        Principal::from_text("zuh6g-qnmvg-vky2t-tnob7-h4xoj-ykrcx-jqjpi-cdf3k-23i3i-ykozs-fae")
            .unwrap();
    /// The gateway registered in the local PocketIc env
    pub static ref GATEWAY_1: Principal =
        Principal::from_text("i3gux-m3hwt-5mh2w-t7wwm-fwx5j-6z6ht-hxguo-t4rfw-qp24z-g5ivt-2qe")
            .unwrap();
    pub static ref GATEWAY_2: Principal =
        Principal::from_text("trj6m-u7l6v-zilnb-2hl6a-3jfz3-asri5-mkw3k-e2tpo-5emmk-6hqxb-uae")
            .unwrap();
}

lazy_static! {
    pub static ref CLIENT_1_KEY: ClientKey = generate_client_key(*CLIENT_1.deref());
    pub static ref CLIENT_2_KEY: ClientKey = generate_client_key(*CLIENT_2.deref());
}

fn generate_client_key(client_principal: Principal) -> ClientKey {
    ClientKey::new(client_principal, generate_random_client_nonce())
}

pub fn generate_random_client_nonce() -> u64 {
    rand::random()
}
