use super::*;

#[test]
fn render_scheduler_adapts_interval_during_streaming() {
    let mut rs = RenderScheduler::new();
    rs.set_streaming(true);

    rs.add_tokens(4);
    rs.mark_dirty();
    rs.last_render = Instant::now() - Duration::from_millis(200);
    let _ = rs.should_render();
    assert_eq!(rs.interval, Duration::from_millis(16));

    rs.add_tokens(2);
    rs.mark_dirty();
    rs.last_render = Instant::now() - Duration::from_millis(200);
    let _ = rs.should_render();
    assert_eq!(rs.interval, Duration::from_millis(60));

    rs.did_render();
    rs.set_streaming(true);
    rs.add_tokens(20);
    rs.mark_dirty();
    rs.last_render = Instant::now() - Duration::from_millis(200);
    let _ = rs.should_render();
    assert_eq!(rs.interval, Duration::from_millis(120));
}

#[test]
fn render_scheduler_non_streaming_is_fast() {
    let mut rs = RenderScheduler::new();
    assert!(!rs.streaming);
    rs.add_tokens(100);
    rs.mark_dirty();
    rs.last_render = Instant::now() - Duration::from_millis(200);
    let _ = rs.should_render();
    assert_eq!(rs.interval, Duration::from_millis(16));
}
