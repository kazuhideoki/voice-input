//! パフォーマンステスト
//! メモリモードの性能を計測します。
//!
//! ## 実行前要件
//! 1. OpenAI APIキーの設定:
//!    ```bash
//!    export OPENAI_API_KEY="your_api_key_here"
//!    ```
//! 2. 音声入力デバイスが利用可能であること
//!    ```bash
//!    cargo run --bin voice_inputd &
//!    cargo run --bin voice_input -- --list-devices
//!    pkill voice_inputd
//!    ```
//!
//! ## 実行方法
//! ```bash
//! # 環境変数を設定してからテスト実行
//! export OPENAI_API_KEY="your_api_key_here"
//! cargo test --test performance_test -- --ignored --nocapture
//! ```

use std::env;
use std::error::Error;
use std::thread;
use std::time::{Duration, Instant};
use voice_input::domain::recorder::Recorder;
use voice_input::infrastructure::audio::cpal_backend::CpalAudioBackend;
use voice_input::infrastructure::external::openai::OpenAiClient;

#[derive(Debug)]
struct PerformanceMetrics {
    recording_time: Duration,
    transcription_time: Duration,
    total_time: Duration,
    #[allow(dead_code)]
    memory_usage_mb: f64,
}

/// 現在のメモリ使用量を取得（簡易実装）
fn get_current_memory_usage_mb() -> f64 {
    // macOSでは正確なメモリ使用量の取得は困難なため、ダミー値を返す
    // 実際の実装では、システムコールやプロセス情報を使用
    0.0
}

/// パフォーマンスを測定
async fn measure_performance() -> Result<PerformanceMetrics, Box<dyn Error>> {
    // 常にメモリモードで計測

    let start = Instant::now();

    // 録音開始
    let backend = CpalAudioBackend::default();
    let mut recorder = Recorder::new(backend);
    recorder.start()?;

    // 5秒間録音
    thread::sleep(Duration::from_secs(5));

    let recording_end = Instant::now();
    let audio_data = recorder.stop_raw()?;

    // OpenAI API呼び出し
    let client = OpenAiClient::new()?;
    let transcription_start = Instant::now();
    let _result = client.transcribe_audio(audio_data).await?;

    let total_end = Instant::now();

    Ok(PerformanceMetrics {
        recording_time: recording_end - start,
        transcription_time: total_end - transcription_start,
        total_time: total_end - start,
        memory_usage_mb: get_current_memory_usage_mb(),
    })
}

#[tokio::test]
#[ignore]
async fn test_performance() {
    // OpenAI APIキーが設定されているか確認
    if env::var("OPENAI_API_KEY").is_err() {
        eprintln!("⚠️  OPENAI_API_KEY not set. Skipping performance test.");
        return;
    }

    println!("🚀 Starting performance test...");
    println!("This test will record 5 seconds of audio in memory mode.\n");

    let metrics = match measure_performance().await {
        Ok(metrics) => metrics,
        Err(e) => {
            eprintln!("❌ Performance test failed: {}", e);
            return;
        }
    };

    println!("Recording Time: {} ms", metrics.recording_time.as_millis());
    println!(
        "Transcription Time: {} ms",
        metrics.transcription_time.as_millis()
    );
    println!("Total Time: {} ms", metrics.total_time.as_millis());
}

#[tokio::test]
#[ignore]
async fn test_memory_usage() {
    println!("\n🧪 Memory Usage Test");
    println!("Testing memory consumption with longer recording...\n");

    // 30秒録音でのメモリ使用量を確認

    let backend = CpalAudioBackend::default();

    let mut recorder = Recorder::new(backend);

    println!("🎙️  Recording for 30 seconds...");
    if let Err(e) = recorder.start() {
        eprintln!("❌ Failed to start recording: {}", e);
        return;
    }

    // 30秒録音
    thread::sleep(Duration::from_secs(30));

    match recorder.stop_raw() {
        Ok(audio_data) => {
            match audio_data {
                voice_input::infrastructure::audio::cpal_backend::AudioData::Memory(data) => {
                    let size_mb = data.len() as f64 / (1024.0 * 1024.0);
                    println!("✅ Memory mode - WAV data size: {:.2} MB", size_mb);

                    // 理論値との比較
                    // 48kHz * 2ch * 2bytes * 30sec = 5.76MB
                    let expected_mb = 48000.0 * 2.0 * 2.0 * 30.0 / (1024.0 * 1024.0);
                    println!("📐 Expected size (theoretical): {:.2} MB", expected_mb);
                    println!(
                        "📊 Actual vs Expected: {:.1}%",
                        (size_mb / expected_mb) * 100.0
                    );
                }
                _ => {}
            }
        }
        Err(e) => {
            eprintln!("❌ Failed to stop recording: {}", e);
        }
    }
}
