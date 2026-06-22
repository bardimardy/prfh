use prfh::app::App;
use prfh::game::writing::Direction;

#[test]
fn typing_up_in_world_mode_changes_direction_immediately() {
    let mut app = App::new_single();
    assert_eq!(app.local_engine().unwrap().direction, Direction::Right);

    app.on_char('u');
    assert_eq!(app.local_engine().unwrap().direction, Direction::Right, "after 'u' still right");

    app.on_char('p');
    assert_eq!(app.local_engine().unwrap().direction, Direction::Up, "after 'p' direction should be Up");
    assert!(
        app.trigger_banner.is_some(),
        "trigger banner should be set after firing"
    );
}

#[test]
fn typing_down_in_world_mode() {
    let mut app = App::new_single();
    for c in "down".chars() {
        app.on_char(c);
    }
    assert_eq!(app.local_engine().unwrap().direction, Direction::Down);
}

#[test]
fn typing_left_then_right() {
    let mut app = App::new_single();
    for c in "left".chars() {
        app.on_char(c);
    }
    assert_eq!(app.local_engine().unwrap().direction, Direction::Left);
    for c in "right".chars() {
        app.on_char(c);
    }
    assert_eq!(app.local_engine().unwrap().direction, Direction::Right);
}

#[test]
fn cursor_advances_up_after_up_trigger() {
    let mut app = App::new_single();
    app.on_char('u');
    app.on_char('p');
    let after_up = app.local_engine().unwrap().cursor;
    app.on_char('x');
    assert_eq!(
        app.local_engine().unwrap().cursor,
        (after_up.0, after_up.1 - 1),
        "next char after 'up' should step upward (y - 1)"
    );
}

#[test]
fn space_does_nothing() {
    let mut app = App::new_single();
    let before_cursor = app.local_engine().unwrap().cursor;
    let before_trail = app.local_engine().unwrap().trail.len();
    app.on_char(' ');
    assert_eq!(app.local_engine().unwrap().cursor, before_cursor, "space must not move cursor");
    assert_eq!(
        app.local_engine().unwrap().trail.len(),
        before_trail,
        "space must not write a tile"
    );
    assert_eq!(
        app.local_engine().unwrap().direction,
        Direction::Right,
        "space must not change direction"
    );
}

#[test]
fn enter_in_world_mode_is_noop() {
    let mut app = App::new_single();
    let before_cursor = app.local_engine().unwrap().cursor;
    let before_dir = app.local_engine().unwrap().direction;
    app.on_enter();
    assert_eq!(app.local_engine().unwrap().cursor, before_cursor);
    assert_eq!(app.local_engine().unwrap().direction, before_dir);
}
