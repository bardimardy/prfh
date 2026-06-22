use prfh::app::App;
use prfh::game::writing::Direction;

#[test]
fn typing_up_in_world_mode_changes_direction_immediately() {
    let mut app = App::new();
    assert_eq!(app.writing.direction, Direction::Right);

    app.on_char('u');
    assert_eq!(app.writing.direction, Direction::Right, "after 'u' still right");

    app.on_char('p');
    assert_eq!(app.writing.direction, Direction::Up, "after 'p' direction should be Up");
    assert!(
        app.trigger_banner.is_some(),
        "trigger banner should be set after firing"
    );
}

#[test]
fn typing_down_in_world_mode() {
    let mut app = App::new();
    for c in "down".chars() {
        app.on_char(c);
    }
    assert_eq!(app.writing.direction, Direction::Down);
}

#[test]
fn typing_left_then_right() {
    let mut app = App::new();
    for c in "left".chars() {
        app.on_char(c);
    }
    assert_eq!(app.writing.direction, Direction::Left);
    for c in "right".chars() {
        app.on_char(c);
    }
    assert_eq!(app.writing.direction, Direction::Right);
}

#[test]
fn cursor_advances_up_after_up_trigger() {
    let mut app = App::new();
    app.on_char('u');
    app.on_char('p');
    let after_up = app.writing.cursor;
    app.on_char('x');
    assert_eq!(
        app.writing.cursor,
        (after_up.0, after_up.1 - 1),
        "next char after 'up' should step upward (y - 1)"
    );
}

#[test]
fn space_does_nothing() {
    let mut app = App::new();
    let before_cursor = app.writing.cursor;
    let before_trail = app.writing.trail.len();
    app.on_char(' ');
    assert_eq!(app.writing.cursor, before_cursor, "space must not move cursor");
    assert_eq!(
        app.writing.trail.len(),
        before_trail,
        "space must not write a tile"
    );
    assert_eq!(
        app.writing.direction,
        Direction::Right,
        "space must not change direction"
    );
}

#[test]
fn enter_in_world_mode_is_noop() {
    let mut app = App::new();
    let before_cursor = app.writing.cursor;
    let before_dir = app.writing.direction;
    app.on_enter();
    assert_eq!(app.writing.cursor, before_cursor);
    assert_eq!(app.writing.direction, before_dir);
}
