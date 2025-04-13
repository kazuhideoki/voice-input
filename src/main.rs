use std::path::Path;

use arboard::Clipboard;
use audio_recoder::RECORDING_STATUS_FILE;
use tokio::runtime::Runtime;

mod audio_recoder;
mod key_monitor;
mod request_speech_to_text;
mod text_selection;
mod transcribe_audio;

use key_monitor::{start_key_monitor, wait_for_stop_trigger};
use request_speech_to_text::{start_recording, stop_recording_and_transcribe};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    if Path::new(RECORDING_STATUS_FILE).exists() {
        // 起動と停止を同じコマンドで実行するため、録音中の場合は処理をスキップ。
        return Ok(());
    }
    dotenv::dotenv().ok();
    println!("Environment variables loaded");

    // Tokio ランタイムの作成
    let rt = Runtime::new()?;

    println!("Starting recording...");
    rt.block_on(start_recording())?;
    println!("Recording started! Press Alt+8 key anywhere to stop recording!");

    // キー監視の開始
    let (stop_trigger, monitor_handle) = start_key_monitor();
    // 停止トリガーを待つ
    wait_for_stop_trigger(&stop_trigger);
    // 監視スレッドの終了待ち
    monitor_handle.join().unwrap();

    println!("Starting to process recording stop...");
    let transcription = rt.block_on(stop_recording_and_transcribe())?;
    println!("{}", transcription);

    let mut clipboard = Clipboard::new().unwrap();
    clipboard.set_text(transcription).unwrap();

    Ok(())
}
