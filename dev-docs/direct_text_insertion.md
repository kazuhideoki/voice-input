# カーソル位置直接テキスト挿入：AppleScript keystrokeアプローチ

## 概要

音声認識結果をコピー&ペーストではなく、カーソル位置に直接入力する方法の調査・設計・実装。

## 現在の問題点

現在の実装（`src/bin/voice_inputd.rs:372-376`）：

```rust
let _ = tokio::process::Command::new("osascript")
    .arg("-e")
    .arg(r#"tell app "System Events" to keystroke "v" using {command down}"#)
    .output()
    .await;
```

**問題：**

- クリップボードの汚染（元の内容が失われる）
- ⌘V操作はクリップボード全体を対象とする

## 解決策：AppleScript keystroke直接入力

### 実装方針

AppleScriptの`keystroke`機能を使用してテキストを直接入力します。

**メリット：**

- ✅ クリップボードを使わない
- ✅ 既存のosascript基盤を活用
- ✅ アプリケーション非依存
- ✅ 日本語・特殊文字対応
- ✅ 実装が簡単

**デメリット：**

- ⚠️ 長いテキストは分割送信が必要
- ⚠️ 文字単位入力のため速度がやや遅い

### 技術実装

#### 1. エスケープ関数

```rust
fn escape_for_applescript(text: &str) -> String {
    text.replace("\\", "\\\\")
        .replace("\"", "\\\"")
        .replace("\n", "\r")  // AppleScriptは\rを改行として認識
        .replace("\r\r", "\r") // 重複回避
}
```

#### 2. 直接入力関数

```rust
async fn type_text_directly(text: &str) -> Result<(), Box<dyn std::error::Error>> {
    const MAX_CHUNK_SIZE: usize = 200; // AppleScript文字数制限対策

    let escaped = escape_for_applescript(text);

    // 長いテキストは分割して送信
    for chunk in escaped.chars().collect::<Vec<_>>().chunks(MAX_CHUNK_SIZE) {
        let chunk_str: String = chunk.iter().collect();
        let script = format!(
            r#"tell application "System Events" to keystroke "{}""#,
            chunk_str
        );

        tokio::process::Command::new("osascript")
            .arg("-e")
            .arg(script)
            .output()
            .await?;

        // 分割送信時の小さな遅延
        if escaped.len() > MAX_CHUNK_SIZE {
            tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;
        }
    }

    Ok(())
}
```

#### 3. voice_inputd.rs での統合

```rust
// handle_transcription関数内の修正
if paste {
    tokio::time::sleep(Duration::from_millis(80)).await;

    if direct_input {
        // 新しい直接入力方式
        if let Err(e) = type_text_directly(&replaced).await {
            eprintln!("Direct input failed: {}, falling back to paste", e);
            // フォールバック: 既存のペースト方式
            let _ = tokio::process::Command::new("osascript")
                .arg("-e")
                .arg(r#"tell app "System Events" to keystroke "v" using {command down}"#)
                .output()
                .await;
        }
    } else {
        // 既存のペースト方式
        let _ = tokio::process::Command::new("osascript")
            .arg("-e")
            .arg(r#"tell app "System Events" to keystroke "v" using {command down}"#)
            .output()
            .await;
    }
}
```

### CLI拡張

#### IpcCmd拡張

```rust
// src/ipc.rs
#[derive(Serialize, Deserialize, Debug)]
pub enum IpcCmd {
    Start {
        paste: bool,
        prompt: Option<String>,
        direct_input: bool, // 新しいフラグ
    },
    // ... 他のコマンドも同様に拡張
}
```

#### CLI引数拡張

```rust
// src/main.rs
#[derive(Subcommand)]
enum Cmd {
    Start {
        #[arg(long, default_value_t = false)]
        paste: bool,
        #[arg(long)]
        prompt: Option<String>,
        #[arg(long, default_value_t = false)]
        direct_input: bool, // 新しいフラグ
    },
    // ... 他のコマンドも同様
}
```

## 段階的実装計画

### Phase 1: 基本実装

1. ✅ 設計文書作成
2. 🔄 AppleScript keystroke関数実装
3. ⏳ voice_inputd.rsへの統合
4. ⏳ 基本テスト

### Phase 2: CLI拡張

1. ⏳ IpcCmd構造体拡張
2. ⏳ CLI引数追加
3. ⏳ エンドツーエンドテスト

### Phase 3: 最適化

1. ⏳ パフォーマンステスト
2. ⏳ エラーハンドリング改善
3. ⏳ 長文分割最適化

## テスト計画

### 基本動作テスト

- [ ] 短いテキスト（1-5語）
- [ ] 中程度のテキスト（1-3文）
- [ ] 長いテキスト（段落レベル）
- [ ] 特殊文字（記号、絵文字）
- [ ] 改行を含むテキスト

### アプリケーション互換性テスト

- [ ] VS Code
- [ ] TextEdit
- [ ] Safari（フォーム入力）
- [ ] Chrome（フォーム入力）
- [ ] Terminal
- [ ] Messages
- [ ] Notes

### パフォーマンステスト

- [ ] 入力遅延測定
- [ ] 長文入力時間測定
- [ ] リソース使用量確認

## 設定オプション

将来的にAppConfigで制御可能にする設定：

```rust
pub struct AppConfig {
    // 既存設定...

    /// デフォルトで直接入力を使用するか
    pub use_direct_input_by_default: bool,

    /// 直接入力の分割サイズ
    pub direct_input_chunk_size: usize,

    /// 分割送信時の遅延（ミリ秒）
    pub direct_input_chunk_delay_ms: u64,

    /// 直接入力失敗時にペーストにフォールバックするか
    pub fallback_to_paste: bool,
}
```

## 既知の制限事項

1. **AppleScript文字数制限**

   - 対策: 文字列分割送信

2. **入力速度**

   - keystrokeは文字単位送信のため、ペーストより遅い
   - 体感的には問題ないレベルと予想

3. **アプリケーション固有の挙動**

   - 一部のアプリでkeystrokeが期待通りに動作しない可能性
   - フォールバック機能で対応

4. **アクセシビリティ権限**
   - System Eventsの使用にはアクセシビリティ権限が必要（既存と同じ）

## 事前テスト：AppleScript keystroke文字数制限調査

実装前に文字数制限を調査するテストスクリプト：

```python
#!/usr/bin/env python3
"""
AppleScript keystrokeの文字数制限テストスクリプト

使用方法:
1. TextEditを開いて新規文書を作成
2. カーソルをテキスト入力エリアに置く
3. python3 keystroke_limit_test.py を実行

テスト内容: 50, 100, 200, 500, 1000, 2000文字での動作確認
"""

import subprocess
import time

def escape_for_applescript(text):
    return text.replace('\\', '\\\\').replace('"', '\\"').replace('\n', '\\r')

def test_keystroke(text, description):
    print(f"\n=== {description} ===")
    print(f"文字数: {len(text)}")

    try:
        escaped = escape_for_applescript(text)
        script = f'tell application "System Events" to keystroke "{escaped}"'

        start_time = time.time()
        result = subprocess.run(["osascript", "-e", script],
                              capture_output=True, text=True, timeout=30)
        end_time = time.time()

        if result.returncode == 0:
            print(f"✅ 成功 (実行時間: {end_time - start_time:.2f}秒)")
            return True
        else:
            print(f"❌ 失敗: {result.stderr.strip()}")
            return False
    except Exception as e:
        print(f"❌ エラー: {e}")
        return False

def generate_test_text(length):
    base = "Mixed text: Hello 世界！Special @#$% chars. 日本語と英語のミックス。123456789. "
    repetitions = (length // len(base)) + 1
    return (base * repetitions)[:length]

# テスト実行
test_cases = [50, 100, 200, 500, 1000, 2000]
for length in test_cases:
    text = generate_test_text(length)
    test_keystroke(text, f"{length}文字テスト")
    time.sleep(2)
```

**このテスト結果を基にMAX_CHUNK_SIZEを決定してください。**

## 段階的実装計画（プルリクエスト最適化）

### P1-1: テキスト直接入力コアモジュール

**範囲:** 基本的なkeystroke機能実装
**ファイル:** `src/infrastructure/external/text_input.rs`（新規）

**実装内容:**

```rust
// 基本的なkeystroke機能のみ。例
pub async fn type_text_directly(text: &str) -> Result<(), std::error::Error>
fn escape_for_applescript(text: &str) -> String
```

**注意:** プロジェクトではanyhowクレートを使用せず、標準の`std::error::Error`を使用します。

**PR要件:**

- [ ] 単体テスト実装
- [ ] 文字数制限対応（テスト結果ベース）
- [ ] エラーハンドリング
- [ ] ドキュメントコメント

### P1-2: IPC拡張（direct_inputフラグ）

**範囲:** 内部通信にdirect_inputオプション追加
**ファイル:** `src/ipc.rs`

**変更内容:**

```rust
#[derive(Serialize, Deserialize, Debug)]
pub enum IpcCmd {
    Start {
        paste: bool,
        prompt: Option<String>,
        direct_input: bool,  // 追加
    },
    Toggle {
        paste: bool,
        prompt: Option<String>,
        direct_input: bool,  // 追加
    },
    // 他は変更なし
}
```

**PR要件:**

- [ ] シリアライゼーションテスト
- [ ] 後方互換性確認

### P1-3: voice_inputd統合

**範囲:** デーモンプロセスでの直接入力実装
**ファイル:** `src/bin/voice_inputd.rs`

**変更内容:**

- `handle_transcription`関数にdirect_inputパラメータ追加
- 直接入力とペーストの分岐処理
- フォールバック機能

**PR要件:**

- [ ] 既存ペースト機能の保持
- [ ] エラー時の適切なフォールバック
- [ ] 統合テスト

### P1-4: CLI引数拡張

**範囲:** ユーザーインターフェース拡張
**ファイル:** `src/main.rs`

**新フラグ:**

- `--direct-input`: 直接入力使用
- `--legacy-paste`: 明示的にペースト方式使用

**動作:**

```bash
# デフォルト（直接入力）
voice_input start --paste

# 明示的に直接入力
voice_input start --paste --direct-input

# レガシーペースト方式
voice_input start --paste --legacy-paste

# 競合時はエラー
voice_input start --paste --direct-input --legacy-paste  # エラー
```

**PR要件:**

- [ ] 引数競合チェック
- [ ] ヘルプテキスト更新
- [ ] CLIテスト

### P1-5: モジュール統合・テスト

**範囲:** 全体統合とテスト強化
**ファイル:** `src/infrastructure/external/mod.rs`等

**実装内容:**

- text_inputモジュールのexport
- エンドツーエンドテスト
- パフォーマンステスト

## 各PRの依存関係

```
P1-1 (コアモジュール)
  ↓
P1-2 (IPC拡張) ← P1-3 (voice_inputd統合)
  ↓                ↓
P1-4 (CLI拡張) ←----┘
  ↓
P1-5 (統合テスト)
```

**並行作業可能:** P1-2とP1-3は同時作業可能

## エラーハンドリング方針

プロジェクトではanyhowクレートを使用せず、以下のパターンでエラーハンドリングを行います：

- **外部ライブラリとの境界**: `Result<T, Box<dyn std::error::Error>>`
- **内部API**: 必要に応じて独自のエラー型を定義
- **文字列エラー**: 簡単なケースでは`&'static str`や`String`

**参考実装**: `src/infrastructure/external/openai.rs:32`

## 次のステップ

1. ✅ 段階的実装計画完成
2. ✅ エラーハンドリング方針確認
3. 🔄 keystroke制限テスト実行（推奨）
4. ⏳ P1-1から順次実装開始

このアプローチにより、適切なPRサイズで段階的に機能を実装できます。
