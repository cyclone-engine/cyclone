# Cyclone Roadmap (Giai đoạn khởi đầu)

> Mục tiêu của Cyclone ở giai đoạn đầu **không phải** là thay thế Unity, Unreal hay Godot.
> Cyclone tập trung trở thành một **headless deterministic multiplayer server framework** với khả năng đồng bộ trạng thái hiệu quả và dễ triển khai trên môi trường cloud/container.

---

# v0.1 - Simulation Foundation

Đây là nền móng của toàn bộ framework.

## Mục tiêu

- Xây dựng deterministic tick simulation.
- Thiết kế World và Entity hoàn toàn tách biệt gameplay.
- Tạo kiến trúc đủ tổng quát để nhiều loại game có thể sử dụng.

## Thành phần

- [x] Tick Scheduler
- [x] World
- [x] Entity
- [x] Entity Lifecycle
- [x] Fixed Tick Loop
- [x] Object-based Architecture

## Kết quả

Đến cuối phiên bản này, Cyclone có thể chạy một thế giới game hoàn chỉnh mà chưa cần networking.

```
Input
    ↓
Tick
    ↓
World
    ↓
Entity
```

## Future Work

Các module sau chưa nằm trong phạm vi của v0.1:

- Snapshot System
- Delta Replication
- Network Transport
- Prediction
- Rollback
- Parallel Tick
- Storage Optimization

Chúng sẽ được thiết kế sau khi simulation runtime đã ổn định và có benchmark thực tế.

### Note

- v0.1 sử dụng `Vec<Box<dyn Object>>` để ưu tiên sự đơn giản và hoàn thiện kiến trúc.
- Storage này không phải thiết kế cuối cùng.
- Sau khi framework ổn định và có benchmark thực tế, Cyclone sẽ đánh giá lại storage backend (SoA, archetype, enum dispatch hoặc các hướng khác) dựa trên dữ liệu đo đạc thay vì giả định.
---

↓

# v0.2 - State Replication

Sau khi simulation đã ổn định, bắt đầu giải quyết multiplayer.

## Mục tiêu

- Đồng bộ trạng thái giữa server và client.
- Chỉ gửi dữ liệu thay đổi (Delta Replication).
- Giảm tối đa băng thông.

## Thành phần

- [x] Snapshot System
- [x] Delta Snapshot
- [x] Entity State Serialization
- [x] Replication Framework
- [x] Interest Management (nếu cần)

## Kết quả

Server có thể tạo snapshot của toàn bộ World và chỉ gửi phần thay đổi tới client.

```
World

↓

Snapshot

↓

Delta

↓

Network Packet
```

---

↓

# v0.3 - Networking & Deployment

Khi simulation và replication đã ổn định, hoàn thiện server runtime.

## Mục tiêu

- Biến Cyclone thành một headless multiplayer server thực sự.
- Có thể triển khai trên Docker/Kubernetes.
- Hỗ trợ scale nhiều room/game server.

## Thành phần

- [x] UDP Transport
- [x] Session Management
- [x] Connection Management
- [x] Packet Dispatcher
- [x] Docker Support
- [x] Container Deployment
- [x] Graceful Shutdown
- [x] Health Check

## Kết quả

Cyclone trở thành một multiplayer server runtime có thể chạy độc lập.

```
Client

↓

UDP

↓

Session

↓

Tick

↓

World

↓

Snapshot

↓

Delta

↓

Client
```

---

# Chưa thực hiện ở giai đoạn này

Các mục dưới đây **không nằm trong mục tiêu ban đầu**, nhằm giữ phạm vi dự án nhỏ và tập trung.

- Unity SDK
- Unreal SDK
- Godot SDK
- Gameplay Code Generation
- IDL / Compiler
- Prediction Code Generation
- Editor
- Client Engine
- Rendering
- UI

Những tính năng này sẽ được nghiên cứu sau khi Cyclone Runtime đã ổn định.

---

# Triết lý phát triển

Ưu tiên giải quyết các vấn đề theo đúng thứ tự:

1. Deterministic Tick Simulation
2. State Replication (Delta)
3. Networking Runtime
4. Container-first Deployment
5. Demo Game (Teeworlds)
6. SDK cho Unity / Unreal / Godot
7. Gameplay Code Generation (tương tự gRPC)

Không mở rộng sang các bài toán khác khi nền tảng simulation chưa đủ vững.


