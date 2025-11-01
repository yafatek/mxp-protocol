use mxp::transport::{
    EndpointRole, Frame, FrameType, PriorityClass, Scheduler, StreamId, StreamKind, StreamManager,
};

#[test]
fn scheduler_respects_priority_and_flow_limits() {
    let mut manager = StreamManager::new(EndpointRole::Client);
    let stream_high = StreamId::new(EndpointRole::Client, StreamKind::Bidirectional, 1);
    let stream_low = StreamId::new(EndpointRole::Client, StreamKind::Bidirectional, 2);

    manager.get_or_create(stream_high);
    manager.get_or_create(stream_low);
    manager.set_connection_limit(8);
    manager.set_stream_limit(stream_high, 6);
    manager.set_stream_limit(stream_low, 4);

    manager.queue_send(stream_high, b"abcdef").unwrap();
    manager.queue_send(stream_low, b"ghij").unwrap();

    let mut scheduler = Scheduler::new();
    scheduler.push_stream(stream_low, PriorityClass::Bulk);
    scheduler.push_stream(stream_high, PriorityClass::Control);

    let (first_id, first_prio) = scheduler.pop_stream().expect("stream available");
    assert_eq!(first_id, stream_high);
    assert_eq!(first_prio, PriorityClass::Control);

    let chunk_high = manager
        .poll_send_chunk(first_id, 16)
        .unwrap()
        .expect("chunk");
    assert_eq!(chunk_high.payload, b"abcdef");
    assert_eq!(manager.stream_send_allowance(stream_high), 0);

    scheduler.push_stream(stream_low, PriorityClass::Bulk);
    let (second_id, _) = scheduler.pop_stream().expect("second stream");
    assert_eq!(second_id, stream_low);
    let chunk_low = manager
        .poll_send_chunk(second_id, 16)
        .unwrap()
        .expect("chunk");
    assert_eq!(chunk_low.payload, b"gh");

    // Simulate receiving flow-control updates via control frames.
    let conn_frame = Frame::connection_max_data(16);
    assert_eq!(conn_frame.frame_type(), FrameType::ConnectionMaxData);
    let new_conn_limit = conn_frame.decode_connection_max_data().unwrap();
    manager.set_connection_limit(new_conn_limit);

    let stream_frame = Frame::stream_max_data(stream_low, 6);
    let (decoded_stream, new_stream_limit) = stream_frame.decode_stream_max_data().unwrap();
    assert_eq!(decoded_stream, stream_low);
    manager.set_stream_limit(decoded_stream, new_stream_limit);

    scheduler.push_stream(stream_low, PriorityClass::Bulk);
    let (third_id, _) = scheduler.pop_stream().expect("third stream");
    assert_eq!(third_id, stream_low);
    let chunk_low_rest = manager
        .poll_send_chunk(third_id, 16)
        .unwrap()
        .expect("remaining chunk");
    assert_eq!(chunk_low_rest.payload, b"ij");
}
