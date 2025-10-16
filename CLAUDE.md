# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

This is ROTS, a Bevy game engine project with procedural terrain generation and physics-based ball spawning. Features include:
- Procedurally generated terrain using Perlin noise
- Avian 3D physics engine integration
- FreeCam/ThirdPerson camera modes with smooth transitions
- Pause system with time scaling
- Interactive ball spawning with physics simulation

## Build and Development Commands

### Nix Environment (Primary)
- **Development shell**: `nix develop` or `direnv allow` (uses flake.nix)
- **Build**: `nix build` (creates `result/bin/bevy2025`)
- **Run**: `nix run` or `./result/bin/bevy2025` after building

### Cargo (Alternative)
- **Build debug**: `cargo build`
- **Build release**: `cargo build --release`
- **Run**: `cargo run`
- **Format**: `cargo fmt`
- **Lint**: `cargo clippy`

## Architecture

### Project Structure
- Single-file application in `src/main.rs`
- Uses Bevy 0.17 game engine (Rust 2024 edition)
- Core systems:
  - `setup`: Initializes 3D scene with camera, lighting, and boundary planes
  - `bouncing_raycast`: Main update loop handling ray casting and bouncing logic

### Key Components
- **Terrain Generation**: Procedural heightmap using Perlin noise with configurable parameters
- **Physics**: Avian3d 0.4 integration with trimesh collider for terrain, sphere colliders for balls
- **Camera System**: FreeCam with WASD+mouse controls, ThirdPerson mode, smooth state transitions
- **Input**: Right-click mouse pan, scrollwheel zoom, spacebar ball spawning, Esc pause toggle
- **Time Scaling**: GameTimeScale resource for pause/slow-motion (infrastructure in place)

### Bevy 0.17 Specifics
This project uses Bevy 0.17, which includes significant changes from earlier versions:
- **Event/Message System**: Uses the new event system architecture (events vs buffered messages)
- **Rendering**: Updated rendering pipeline with reorganized crates
- **System Execution**: More conservative parallelism and `Result`-based error handling
- **Component API**: Uses current component and entity relationship patterns

### Technical Details
- Optimized debug profile with level 1 optimization for main code, level 3 for dependencies
- Requires graphics libraries (Vulkan, Wayland, ALSA) on Linux via Nix flake
- Cross-platform build support for Linux and Windows

## CI/CD

The project has automated version bumping and cross-platform builds:
- Automatic version bumping based on commit messages `[major]`, `[minor]`, or `[patch]`
- Linux builds via Nix
- Windows builds via cargo
- GitHub releases with both platform binaries

## Physics (Avian 0.4)

Uses Avian3d 0.4 physics engine. Key API patterns:
- `PhysicsPlugins::default()` for setup, `Gravity(Vec3)` resource
- `RigidBody::{Static, Dynamic}`, `Collider::sphere/trimesh_from_mesh`
- `Mass(f32)` component for dynamic bodies
- Collision events: `CollisionStart`/`CollisionEnd` (renamed from Started/Ended in 0.3)
- Force API: Use `ConstantForce`/`ConstantTorque` or `Forces` helper (ExternalForce removed in 0.4)
- System sets renamed to `*Systems` pattern (e.g., `PhysicsSystems`)

## Bevy Migration Notes (0.15→0.17)
0.15→0.16: Query::single()→Result, Parent→ChildOf, EventWriter::send()→write(), systems return Result, no-std support. 0.16→0.17: EventWriter/Reader→MessageWriter/Reader, rendering moved to bevy_camera/shader/light, Handle::Weak→Handle::Uuid, RenderStartup vs Plugin::finish, StateScoped→DespawnOnExit, cursor types bevy_winit→bevy_window, conservative system parallelism, web RUSTFLAGS required.