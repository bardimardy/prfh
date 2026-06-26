//! Dünner tachyonfx-Wrapper: benannte Effekt-Konstruktoren.
//!
//! tachyonfx ist visuell/zeitgetrieben — keine echten Unit-Tests, sondern
//! „konstruiert + bis zum Ende prozessiert ohne Panic"-Smoke-Tests gegen einen
//! Scratch-Buffer. `main` wird NICHT auf visuelle Korrektheit gegated.

use ratatui::style::{Color, Style};
use tachyonfx::fx::ExpandDirection;
use tachyonfx::{fx, Effect, Interpolation, Motion};

/// `expand`/`stretch` paniken bei Overshoot-Easings (Back*/Elastic*) durch
/// Subtraktions-Overflow. Dieser Guard ist die zentrale Leitplanke der
/// Non-Overshoot-Regel — er hält `safe_expand` auf sichere Kurven.
fn is_non_overshoot(c: Interpolation) -> bool {
    !matches!(
        c,
        Interpolation::BackIn
            | Interpolation::BackOut
            | Interpolation::BackInOut
            | Interpolation::ElasticIn
            | Interpolation::ElasticOut
            | Interpolation::ElasticInOut
            | Interpolation::BounceIn
            | Interpolation::BounceOut
            | Interpolation::BounceInOut
            | Interpolation::Spring
    )
}

/// Einziger erlaubter Weg, im effects-Modul `expand` zu bauen. Der
/// `debug_assert!` verhindert, dass je eine Overshoot-Kurve durchrutscht.
fn safe_expand(dir: ExpandDirection, style: Style, ms: u32, curve: Interpolation) -> Effect {
    debug_assert!(
        is_non_overshoot(curve),
        "expand panik-Regel verletzt: Overshoot-Kurve {curve:?} ist verboten"
    );
    fx::expand(dir, style, (ms, curve))
}

/// Pickup eines Powerup-Worts: kurzer, freundlicher Farb-Puls + Slide-In.
/// Verwendet nur verifizierte Bausteine (`parallel`, `hsl_shift`, `slide_in`).
pub fn pickup() -> Effect {
    fx::parallel(&[
        fx::hsl_shift(
            Some([90.0, 25.0, 15.0]),
            None,
            (600, Interpolation::SineOut),
        ),
        fx::slide_in(Motion::UpToDown, 6, 0, Color::Black, 600),
    ])
}

/// Aktivierung eines Powerups: horizontale Welle (`expand`) gefolgt von einem
/// Farb-Shift. `expand` läuft bewusst über `safe_expand` mit `CircOut` — einer
/// Non-Overshoot-Kurve — und darf daher nicht paniken.
pub fn activation() -> Effect {
    fx::sequence(&[
        safe_expand(
            ExpandDirection::Horizontal,
            Style::default().bg(Color::Indexed(54)),
            500,
            Interpolation::CircOut,
        ),
        fx::hsl_shift(
            Some([200.0, 20.0, 10.0]),
            None,
            (400, Interpolation::QuadOut),
        ),
    ])
}

/// Dash-Lande-Pop: kurzer, lokalisierter Effekt am Lande-Tile (kleines Rect) —
/// die Zeichen sammeln sich (`coalesce`) mit warmem Hue-Shift. Bewusst KEIN
/// `explode`/`evolve` (die blanken das Feld). Wird im hud_lab über ein kleines
/// Rect prozessiert; In-Game übernimmt die zentrierte Cast-Welle den Pop.
pub fn dash_landing() -> Effect {
    fx::parallel(&[
        fx::coalesce((250, Interpolation::SineOut)),
        fx::hsl_shift_fg([60.0, 30.0, 40.0], (250, Interpolation::QuadOut)),
    ])
}

/// Notification-Panel-Aufbau: horizontale Welle aus der Mitte (`expand`), die
/// das Panel beim Reinkommen „aufzieht". Läuft über `safe_expand` mit `CircOut`
/// (Non-Overshoot) — darf nicht paniken. Stil ist die Panel-Füllung.
pub fn notify_panel(ms: u32) -> Effect {
    safe_expand(
        ExpandDirection::Horizontal,
        Style::default().bg(crate::theme::PANEL_BG),
        ms,
        Interpolation::CircOut,
    )
}

/// Notification-Text-Enthüllung: die Zeichen sammeln sich aus Streuung zum
/// lesbaren Text (`coalesce`). Wird über das Text-Rect prozessiert, nachdem der
/// volle Text bereits gesetzt wurde.
pub fn notify_reveal(ms: u32) -> Effect {
    fx::coalesce((ms, Interpolation::SineOut))
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::buffer::Buffer;
    use ratatui::layout::Rect;
    use std::time::Duration;
    use tachyonfx::EffectManager;

    /// Prozessiert einen Effekt über denselben Pfad wie der Render-Hook weit
    /// über seine Timer-Dauer hinaus. Paniken (z.B. expand-Overflow) schlagen
    /// hier zu — genau das ist der Sinn.
    fn run_to_end(effect: tachyonfx::Effect) {
        let mut mgr: EffectManager<()> = EffectManager::default();
        mgr.add_effect(effect);
        let mut buf = Buffer::empty(Rect::new(0, 0, 24, 12));
        let area = buf.area;
        let step = Duration::from_millis(50);
        // 200 * 50ms = 10s — länger als jeder Konstruktor-Effekt.
        for _ in 0..200 {
            mgr.process_effects(step.into(), &mut buf, area);
        }
    }

    #[test]
    fn pickup_runs_to_end_without_panic() {
        run_to_end(pickup());
    }

    #[test]
    fn activation_runs_to_end_without_panic() {
        run_to_end(activation());
    }

    #[test]
    fn notify_panel_runs_to_end_without_panic() {
        // expand-Panik-Regel: muss über die volle Timer-Dauer hinaus laufen.
        run_to_end(notify_panel(240));
    }

    #[test]
    fn notify_reveal_runs_to_end_without_panic() {
        run_to_end(notify_reveal(260));
    }

    #[test]
    fn guard_rejects_overshoot_curves() {
        assert!(!is_non_overshoot(Interpolation::BackOut));
        assert!(!is_non_overshoot(Interpolation::ElasticOut));
        assert!(is_non_overshoot(Interpolation::CircOut));
        assert!(is_non_overshoot(Interpolation::QuadOut));
        assert!(is_non_overshoot(Interpolation::SineOut));
        assert!(is_non_overshoot(Interpolation::CubicOut));
    }

    #[test]
    fn guard_rejects_bounce_and_spring() {
        assert!(!is_non_overshoot(Interpolation::BounceOut));
        assert!(!is_non_overshoot(Interpolation::BounceIn));
        assert!(!is_non_overshoot(Interpolation::BounceInOut));
        assert!(!is_non_overshoot(Interpolation::Spring));
    }

    #[test]
    fn dash_landing_runs_to_end_without_panic() {
        run_to_end(dash_landing());
    }
}
