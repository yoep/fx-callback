# FX-Callback
![Build](https://github.com/yoep/fx-callback/workflows/Build/badge.svg)
![Version](https://img.shields.io/github/v/tag/yoep/fx-callback?label=version)
[![Crates](https://img.shields.io/crates/v/fx-callback)](https://crates.io/crates/fx-callback)
[![License: Apache-2.0](https://img.shields.io/github/license/yoep/fx-callback)](./LICENSE)
[![codecov](https://codecov.io/gh/yoep/fx-callback/branch/master/graph/badge.svg?token=A801IOOZAH)](https://codecov.io/gh/yoep/fx-callback)

A subscription based callback for data events that might occur within one or more structs.
It is mainly used within the FX landscape to allow events to be published between multiple structs.

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

    fn subscribe_with(&self, subscriber: Subscriber<MyEvent>) {
        self.callbacks.subscribe_with(subscriber)
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
    
    fn subscribe_with(&self, subscriber: Subscriber<MyEvent>) {
        self.callbacks.subscribe_with(subscriber)
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
use fx_callback::{Callback, MultiThreadedCallback, Subscriber, Subscription};
use tokio::runtime::Runtime;

fn main() {
    let runtime = Runtime::new().unwrap();
    let struct_with_callback = MyStruct::new();
    
    let mut receiver = struct_with_callback.subscribe();
    runtime.spawn(async move {
       loop {
           if let Some(event) = receiver.recv().await {
               println!("Received event: {}", event);
           } else {
               break;
           }
       } 
    });

    struct_with_callback.invoke_event();
}
```