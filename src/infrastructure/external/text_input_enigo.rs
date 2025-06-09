//! Enigoを使用したテキスト直接入力モジュール
//!
//! enigoライブラリを使用して、日本語を含む全ての文字を
//! カーソル位置に直接入力する機能を提供

use enigo::{Enigo, Keyboard, Settings};
use std::error::Error;
use std::fmt;

/// Enigoを使用したテキスト入力に関するエラー
#[derive(Debug)]
pub enum EnigoInputError {
    /// Enigo初期化エラー
    InitError(String),
    /// テキスト入力エラー
    InputError(String),
}

impl fmt::Display for EnigoInputError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            EnigoInputError::InitError(msg) => {
                write!(f, "Enigo initialization failed: {}", msg)
            }
            EnigoInputError::InputError(msg) => {
                write!(f, "Text input failed: {}", msg)
            }
        }
    }
}

impl Error for EnigoInputError {}

/// Enigoを使用してテキストを直接入力
///
/// # Arguments
/// * `text` - 入力するテキスト（日本語対応）
///
/// # Returns
/// 成功時は Ok(()), 失敗時は EnigoInputError
pub async fn type_text_with_enigo(text: &str) -> Result<(), EnigoInputError> {
    // String型にクローンして所有権を移動
    let text_owned = text.to_string();

    // tokioの非同期環境からブロッキング処理を実行
    tokio::task::spawn_blocking(move || {
        // Enigoインスタンスを作成（mac_delayを設定）
        let settings = Settings {
            mac_delay: 20, // キーイベント間の遅延（ミリ秒）
            ..Default::default()
        };

        let mut enigo =
            Enigo::new(&settings).map_err(|e| EnigoInputError::InitError(e.to_string()))?;

        // 少し待機
        std::thread::sleep(std::time::Duration::from_millis(50));

        // Metaキーのみリリース（最小限の修飾キー操作）
        use enigo::{Direction::Release, Key};
        let _ = enigo.key(Key::Meta, Release);

        // リセット後の待機
        std::thread::sleep(std::time::Duration::from_millis(30));

        // テキストを入力
        // enigoのtext()メソッドは、Unicode文字を含む全ての文字を正しく処理
        enigo
            .text(&text_owned)
            .map_err(|e| EnigoInputError::InputError(e.to_string()))?;

        // Enigo処理完了後の待機（rdevの状態回復）
        std::thread::sleep(std::time::Duration::from_millis(30));

        Ok(())
    })
    .await
    .map_err(|e| EnigoInputError::InitError(format!("Task join error: {}", e)))?
}

/// デフォルト設定でテキストを入力
pub async fn type_text_default(text: &str) -> Result<(), Box<dyn Error>> {
    type_text_with_enigo(text).await.map_err(|e| e.into())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    #[ignore] // 手動実行用
    async fn test_enigo_japanese_input() {
        let test_cases = vec![
            "Hello, World!",
            "こんにちは、世界！",
            "日本語のテキスト入力テスト",
            "Mixed text: 英語 and 日本語",
            "特殊文字: @#$% 絵文字: 🎉",
        ];

        for text in test_cases {
            println!("Testing: {}", text);
            match type_text_with_enigo(text).await {
                Ok(_) => println!("✓ Success"),
                Err(e) => println!("✗ Error: {}", e),
            }
            tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
        }
    }
}
