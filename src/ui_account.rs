use std::str::FromStr;

use solana_account_decoder_client_types::UiAccount;
use solana_native_token::LAMPORTS_PER_SOL;
use solana_pubkey::Pubkey;

pub fn lamports_to_sol(lamports: u64) -> f64 {
    lamports as f64 / LAMPORTS_PER_SOL as f64
}

pub fn decode_ui_account_lamports(ui_account: &UiAccount) -> Option<u64> {
    ui_account.data.decode()?;
    Pubkey::from_str(&ui_account.owner).ok()?;
    Some(ui_account.lamports)
}
