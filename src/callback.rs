use std::fmt::Debug;
use std::sync::Arc;
use tokio::sync::broadcast;
use tokio::sync::broadcast::{Receiver, Sender};

/// The subscription type for the interested event.
/// Drop this subscription to remove the callback.
pub type Subscription<T> = Receiver<Arc<T>>;

/// Allows adding callbacks to the struct.
/// The struct will inform the [Subscription] when a certain event occurs.
///
/// # Example
///
/// ```rust,no_run
/// use std::sync::Arc;
/// use tokio::runtime::Runtime;
/// use fx_callback::{Callback, MultiThreadedCallback};
///
/// #[derive(Debug)]
/// pub enum MyEvent {
///     Foo,
///     Bar,
/// }
///
/// async fn register_callback() {
///     let callback = MultiThreadedCallback::<MyEvent>::new();
///     let mut receiver = callback.subscribe();
///
///     let event = receiver.recv().await.unwrap();
///     // do something with the event
/// }
/// ```
pub trait Callback<T>: Debug
where
    T: Debug,
{
    /// Subscribe to the interested event.
    /// This creates a new [Subscription] that will be invoked with a shared instance of the event when the interested event occurs.
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// use fx_callback::Callback;
    ///
    /// #[derive(Debug, Clone, PartialEq)]
    /// pub enum MyEvent {
    ///     Foo,
    /// }
    ///
    /// async fn example(callback: &dyn Callback<MyEvent>) {
    ///     let mut receiver = callback.subscribe();
    ///     
    ///     if let Some(event) = receiver.recv().await {
    ///         // do something with the event
    ///     }
    /// }
    ///
    /// ```
    ///
    /// # Returns
    ///
    /// It returns a [Subscription] which can be dropped to remove the callback.
    fn subscribe(&self) -> Subscription<T>;
}

/// A multithreaded callback holder.
///
/// This callback holder will invoke the given events on a separate thread, thus unblocking the caller thread for other tasks.
///
/// # Example
///
/// ```rust,no_run
/// use fx_callback::{Callback, MultiThreadedCallback, Subscription};
///
/// /// The events of the struct that informs subscribers about changes to the data within the struct.
/// #[derive(Debug, Clone, PartialEq)]
/// enum MyEvent {
///     Foo,
/// }
///
/// /// The struct to which an interested subscriber can subscribe to.
/// #[derive(Debug)]
/// struct Example {
///     callbacks: MultiThreadedCallback<MyEvent>,
/// }
///
/// impl Example {
///     fn invoke_event(&self) {
///         self.callbacks.invoke(MyEvent::Foo);
///     }
/// }
///
/// impl Callback<MyEvent> for Example {
///     fn subscribe(&self) -> Subscription<MyEvent> {
///         self.callbacks.subscribe()
///     }
/// }
/// ```
#[derive(Debug)]
pub struct MultiThreadedCallback<T>
where
    T: Debug,
{
    sender: Sender<Arc<T>>,
}

impl<T> Callback<T> for MultiThreadedCallback<T>
where
    T: Debug,
{
    fn subscribe(&self) -> Subscription<T> {
        self.sender.subscribe()
    }
}

impl<T> Clone for MultiThreadedCallback<T>
where
    T: Debug,
{
    fn clone(&self) -> Self {
        Self {
            sender: self.sender.clone(),
        }
    }
}

impl<T> MultiThreadedCallback<T>
where
    T: Debug + Send + Sync + 'static,
{
    /// Creates a new multithreaded callback.
    pub fn new() -> Self {
        Self::new_with_capacity(512)
    }

    /// Creates a new multithreaded callback with a specified capacity.
    pub fn new_with_capacity(capacity: usize) -> Self {
        let (sender, _) = broadcast::channel(capacity);
        Self { sender }
    }

    /// Invoke the currently registered callbacks and inform them of the given value.
    ///
    /// # Arguments
    ///
    /// * `value` - The value to invoke the callbacks with.
    pub fn invoke(&self, value: T) {
        let _ = self.sender.send(Arc::new(value));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;
    use tokio::{select, time};

    #[derive(Debug, Clone, PartialEq)]
    pub enum Event {
        Foo,
    }

    #[derive(Debug, PartialEq)]
    enum NoneCloneEvent {
        Bar,
    }

    #[tokio::test]
    async fn test_multi_threaded_invoke() {
        init_logger!();
        let expected_result = Event::Foo;
        let (tx, mut rx) = tokio::sync::mpsc::channel(1);
        let callback = MultiThreadedCallback::<Event>::new();

        let mut receiver = callback.subscribe();
        tokio::spawn(async move {
            if let Ok(e) = receiver.recv().await {
                let _ = tx.send(e).await;
            }
        });

        callback.invoke(expected_result.clone());
        let result = select! {
            _ = time::sleep(Duration::from_millis(150)) => {
                panic!("Callback invocation receiver timed out")
            },
            Some(result) = rx.recv() => result,
        };

        assert_eq!(expected_result, *result);
    }

    #[tokio::test]
    async fn test_invoke_dropped_receiver() {
        init_logger!();
        let expected_result = Event::Foo;
        let (tx, mut rx) = tokio::sync::mpsc::channel(1);
        let callback = MultiThreadedCallback::<Event>::new();

        let _ = callback.subscribe();
        let mut receiver = callback.subscribe();
        tokio::spawn(async move {
            if let Ok(e) = receiver.recv().await {
                let _ = tx.send(e).await;
            }
        });

        callback.invoke(expected_result.clone());
        let result = select! {
            _ = time::sleep(Duration::from_millis(150)) => {
                panic!("Callback invocation receiver timed out")
            },
            Some(result) = rx.recv() => result,
        };

        assert_eq!(expected_result, *result);
    }

    #[tokio::test]
    async fn test_non_cloneable_type() {
        init_logger!();
        let (tx, mut rx) = tokio::sync::mpsc::channel(1);
        let callback = MultiThreadedCallback::<NoneCloneEvent>::new();

        let mut receiver = callback.subscribe();
        tokio::spawn(async move {
            if let Ok(e) = receiver.recv().await {
                let _ = tx.send(e).await;
            }
        });

        callback.invoke(NoneCloneEvent::Bar);
        let result = select! {
            _ = time::sleep(Duration::from_millis(150)) => {
                panic!("Callback invocation receiver timed out")
            },
            Some(result) = rx.recv() => result,
        };

        assert_eq!(NoneCloneEvent::Bar, *result);
    }
}
