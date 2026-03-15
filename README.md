# FX-Callback
![Build](https://github.com/yoep/fx-callback/workflows/Build/badge.svg)
![Version](https://img.shields.io/github/v/tag/yoep/fx-callback?label=version)
[![Crates](https://img.shields.io/crates/v/fx-callback)](https://crates.io/crates/fx-callback)
[![License: Apache-2.0](https://img.shields.io/github/license/yoep/fx-callback)](./LICENSE)
[![codecov](https://codecov.io/gh/yoep/fx-callback/branch/master/graph/badge.svg?token=A801IOOZAH)](https://codecov.io/gh/yoep/fx-callback)

As of v2+, the `fx-callback` crate was refactored to be a simple wrapper around the `broadcast` feature of tokio.
If you're still using this crate, it's recommended to switch to `tokio::sync::broadcast`.

## Example

```rust
use fx_callback::{Callback, MultiThreadedCallback, Subscriber, Subscription};

/// The events of the struct that informs subscribers about changes to the data within the struct.
#[derive(Debug, Clone, PartialEq)]
enum MyEvent {
    Foo,
}

/// The struct to which an interested subscriber can subscribe to.
#[derive(Debug)]
struct Example {
    callbacks: MultiThreadedCallback<MyEvent>,
}

impl Example {
    fn invoke_event(&self) {
        self.callbacks.invoke(MyEvent::Foo);
    }
}

impl Callback<MyEvent> for Example {
    fn subscribe(&self) -> Subscription<MyEvent> {
        self.callbacks.subscribe()
    }
}
```

## Usage

### Subscription/event holder

To get started with adding callbacks to your structs, add one of the implementations of the `Callback` trait.
Make sure that the struct implements the `Debug` trait.

```rust
use fx_callback::{Callback, MultiThreadedCallback};

#[derive(Debug)]
pub struct MyStruct {
    callbacks: MultiThreadedCallback<MyEvent>,
}
```

Add the `Callback` trait implementation to your struct to allow adding callbacks.

```rust
impl Callback<MyEvent> for MyStruct {
    fn subscribe(&self) -> Subscription<MyEvent> {
        self.callbacks.subscribe()
    }
}
```

When you want to inform subscribers about a certain event, call the `invoke` method.

```rust
impl MyStruct {
    pub fn invoke_event(&self) {
        self.callbacks.invoke(MyEvent::Foo);
    }
}
```

### Subscriber

The interested subscriber can subscribe to the interested event of a struct that implements the `Callback` trait.

```rust
use fx_callback::{Callback, MultiThreadedCallback, Subscription};
use std::io;

#[tokio::main]
async fn main() -> Result<(), io::Error>{
    let struct_with_callback = MyStruct::new();
    
    let mut receiver = struct_with_callback.subscribe();
    tokio::spawn(async move {
       while let Ok(event) = receiver.recv().await {
           println!("Received event: {}", event);
       }
    });

    struct_with_callback.invoke_event();
    Ok(())
}

#[derive(Debug)]
pub struct MyStruct {
    callbacks: MultiThreadedCallback<MyEvent>,
}

impl Callback<MyEvent> for MyStruct {
    fn subscribe(&self) -> Subscription<MyEvent> {
        self.callbacks.subscribe()
    }
}
```