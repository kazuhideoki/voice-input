//! メディア再生制御サービス
//!
//! # 責任
//! - Apple Musicの再生状態管理
//! - 録音時の自動一時停止/再開

use std::sync::{Arc, Mutex};

use crate::application::traits::MediaController;
use crate::error::{Result, VoiceInputError};
use crate::infrastructure::external::sound::{pause_apple_music, resume_apple_music};

/// メディア制御サービス
pub struct MediaControlService {
    /// 録音によって一時停止されたかを記録
    paused_by_recording: Arc<Mutex<bool>>,
    /// メディアコントローラー（オプショナル：テスト時のモック用）
    controller: Option<Box<dyn MediaController>>,
}

impl MediaControlService {
    /// 新しいMediaControlServiceを作成
    pub fn new() -> Self {
        Self {
            paused_by_recording: Arc::new(Mutex::new(false)),
            controller: None,
        }
    }

    /// カスタムコントローラーで作成（テスト用）
    pub fn with_controller(controller: Box<dyn MediaController>) -> Self {
        Self {
            paused_by_recording: Arc::new(Mutex::new(false)),
            controller: Some(controller),
        }
    }

    /// 再生中の場合は一時停止し、状態を記録
    pub async fn pause_if_playing(&self) -> Result<bool> {
        if let Some(ref controller) = self.controller {
            // モックコントローラーを使用
            if controller.is_playing().await? {
                controller.pause().await?;
                *self
                    .paused_by_recording
                    .lock()
                    .map_err(|e| VoiceInputError::SystemError(format!("Lock error: {}", e)))? =
                    true;
                Ok(true)
            } else {
                Ok(false)
            }
        } else {
            // 実際のApple Music制御を使用
            let was_playing = pause_apple_music();
            *self
                .paused_by_recording
                .lock()
                .map_err(|e| VoiceInputError::SystemError(format!("Lock error: {}", e)))? =
                was_playing;
            Ok(was_playing)
        }
    }

    /// 録音によって一時停止されていた場合は再開
    pub async fn resume_if_paused(&self) -> Result<()> {
        let should_resume = *self
            .paused_by_recording
            .lock()
            .map_err(|e| VoiceInputError::SystemError(format!("Lock error: {}", e)))?;

        if should_resume {
            if let Some(ref controller) = self.controller {
                // モックコントローラーを使用
                controller.resume().await?;
            } else {
                // 実際のApple Music制御を使用
                resume_apple_music();
            }
            *self
                .paused_by_recording
                .lock()
                .map_err(|e| VoiceInputError::SystemError(format!("Lock error: {}", e)))? = false;
        }

        Ok(())
    }

    /// 現在録音によって一時停止中かどうかを確認
    pub fn is_paused_by_recording(&self) -> Result<bool> {
        Ok(*self
            .paused_by_recording
            .lock()
            .map_err(|e| VoiceInputError::SystemError(format!("Lock error: {}", e)))?)
    }

    /// 状態をリセット
    pub fn reset(&self) -> Result<()> {
        *self
            .paused_by_recording
            .lock()
            .map_err(|e| VoiceInputError::SystemError(format!("Lock error: {}", e)))? = false;
        Ok(())
    }
}

impl Default for MediaControlService {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use async_trait::async_trait;
    use std::sync::atomic::{AtomicBool, Ordering};

    /// テスト用のモックメディアコントローラー
    struct MockMediaController {
        playing: Arc<AtomicBool>,
    }

    impl MockMediaController {
        fn new(initial_playing: bool) -> Self {
            Self {
                playing: Arc::new(AtomicBool::new(initial_playing)),
            }
        }
    }

    #[async_trait]
    impl MediaController for MockMediaController {
        async fn is_playing(&self) -> Result<bool> {
            Ok(self.playing.load(Ordering::SeqCst))
        }

        async fn pause(&self) -> Result<()> {
            self.playing.store(false, Ordering::SeqCst);
            Ok(())
        }

        async fn resume(&self) -> Result<()> {
            self.playing.store(true, Ordering::SeqCst);
            Ok(())
        }
    }

    #[tokio::test]
    async fn test_pause_if_playing_when_playing() {
        let controller = Box::new(MockMediaController::new(true));
        let service = MediaControlService::with_controller(controller);

        let was_playing = service.pause_if_playing().await.unwrap();
        assert!(was_playing);
        assert!(service.is_paused_by_recording().unwrap());
    }

    #[tokio::test]
    async fn test_pause_if_playing_when_not_playing() {
        let controller = Box::new(MockMediaController::new(false));
        let service = MediaControlService::with_controller(controller);

        let was_playing = service.pause_if_playing().await.unwrap();
        assert!(!was_playing);
        assert!(!service.is_paused_by_recording().unwrap());
    }

    #[tokio::test]
    async fn test_resume_if_paused() {
        let controller = Box::new(MockMediaController::new(true));
        let playing_ref = controller.playing.clone();
        let service = MediaControlService::with_controller(controller);

        // まず一時停止
        service.pause_if_playing().await.unwrap();
        assert!(!playing_ref.load(Ordering::SeqCst));

        // 再開
        service.resume_if_paused().await.unwrap();
        assert!(playing_ref.load(Ordering::SeqCst));
        assert!(!service.is_paused_by_recording().unwrap());
    }

    #[tokio::test]
    async fn test_resume_if_paused_when_not_paused() {
        let controller = Box::new(MockMediaController::new(false));
        let playing_ref = controller.playing.clone();
        let service = MediaControlService::with_controller(controller);

        // 再開を試みる（何も起こらないはず）
        service.resume_if_paused().await.unwrap();
        assert!(!playing_ref.load(Ordering::SeqCst));
    }
}
