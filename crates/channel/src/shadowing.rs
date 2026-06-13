//! log-normal シャドウイングの設定保持（設計 §4.3「大規模量」共通基盤）。
//!
//! シャドウイングは空間相関した大規模変動で、受信電力へ dB 加算される確率実現量。
//! 本クレートの InF 実装はこれを **per-link 決定論ハッシュ**で引く方式を採る:
//! リンク (cell, ue) と固定シードから直接 N(0,σ²) を導出するため、
//! engine の RNG 列を消費せず、RadioMap の dirty 再計算をまたいでも同一リンクは
//! 常に同一値になる（設計 §8.1 の決定論を呼び出し順非依存で達成）。
//!
//! そのため [`ShadowField`] は σ の保持と有効判定のみを担い、乱数バッファを
//! 持たない。空間相関（38.901 の相関距離 d_corr）を厳密化する将来拡張では、
//! ここに相関行列の Cholesky 等を載せ、`InfChannel` 側の引き方を差し替える。

/// シャドウイングの構成（標準偏差のみ）。`sigma_db == 0` で無効。
#[derive(Debug, Clone, Copy)]
pub struct ShadowField {
    sigma_db: f64,
    /// 場が一度でも参照されたか（静的シナリオの初回 update 通知用フラグ）。
    /// per-link ハッシュ方式では生成バッファを持たないため、初回参照済みを
    /// 表す論理状態としてのみ用いる。
    generated: bool,
}

impl ShadowField {
    pub fn new(sigma_db: f64) -> Self {
        Self {
            sigma_db,
            generated: false,
        }
    }

    #[inline]
    pub fn is_enabled(&self) -> bool {
        self.sigma_db > 0.0
    }

    #[inline]
    pub fn sigma_db(&self) -> f64 {
        self.sigma_db
    }

    #[inline]
    pub fn is_generated(&self) -> bool {
        self.generated
    }

    /// 初回 update で「生成済み」へ遷移させる（変化通知を一度だけ出すため）。
    #[inline]
    pub fn mark_generated(&mut self) {
        self.generated = true;
    }
}
