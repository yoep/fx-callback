use fx_handle::Handle;
use log::{debug, error, trace, warn};
use std::collections::HashMap;
use std::fmt::Debug;
use std::sync::{Arc, Mutex};
use std::time::Instant;
use tokio::runtime::Runtime;
use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender};

/// The unique identifier for a callback.
pub type CallbackHandle = Handle;

/// The subscription type for the interested event.
/// Drop this subscription to remove the callback.
pub type Subscription<T> = UnboundedReceiver<Arc<T>>;

/// The subscriber type for the interested event.
/// This can be used to send the interested event from multiple sources into one receiver.
pub type Subscriber<T> = UnboundedSender<Arc<T>>;

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
    T: Debug + Send + Sync,
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

    /// Subscribe to the interested event with a [Subscriber].
    /// This creates an underlying new subscription which will be invoked with the given subscriber when the interested event occurs.
    ///
    /// ## Remarks
    ///
    /// It is possible to grant multiple subscriptions from the same source to the same interested event,
    /// as the [Callback] is only a holder for the [Subscription] and can't detect any duplicates.
    fn subscribe_with(&self, subscriber: Subscriber<T>);
}

/// A multithreaded callback holder.
///
/// This callback holder will invoke the given events on a separate thread, thus unblocking the caller thread for other tasks.
///
/// # Example
///
/// ```rust,no_run
/// use fx_callback::{Callback, MultiThreadedCallback, Subscriber, Subscription};
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
///
///     fn subscribe_with(&self, subscriber: Subscriber<MyEvent>) {
///         self.callbacks.subscribe_with(subscriber)
///     }
/// }
/// ```
#[derive(Debug, Clone)]
pub struct MultiThreadedCallback<T>
where
    T: Debug + Send + Sync,
{
    base: Arc<BaseCallback<T>>,
    runtime: Arc<Mutex<Option<Runtime>>>,
}

impl<T> Callback<T> for MultiThreadedCallback<T>
where
    T: Debug + Send + Sync,
{
    fn subscribe(&self) -> Subscription<T> {
        self.base.subscribe()
    }

    fn subscribe_with(&self, subscriber: Subscriber<T>) {
        self.base.subscribe_with(subscriber)
    }
}

impl<T> MultiThreadedCallback<T>
where
    T: Debug + Send + Sync + 'static,
{
    /// Creates a new multithreaded callback.
    pub fn new() -> Self {
        Self {
            base: Arc::new(BaseCallback::<T>::new()),
            runtime: Arc::new(Mutex::new(None)),
        }
    }

    /// Invoke the currently registered callbacks and inform them of the given value.
    ///
    /// # Arguments
    ///
    /// * `value` - The value to invoke the callbacks with.
    pub fn invoke(&self, value: T) {
        let inner = self.base.clone();
        match tokio::runtime::Handle::try_current() {
            Ok(_) => {
                // spawn the invocation operation in a new thread
                tokio::spawn(async move {
                    inner.invoke(value);
                });
            }
            Err(_) => match self.runtime.lock() {
                Ok(mut runtime) => {
                    runtime
                        .get_or_insert_with(|| Runtime::new().unwrap())
                        .spawn(async move {
                            inner.invoke(value);
                        });
                }
                Err(e) => error!("Failed to acquire lock: {}", e),
            },
        }
    }
}

/// A single threaded or current threaded callback holder.
///
/// This callback holder will invoke the given events on the current thread, thus blocking the caller thread for other tasks.
#[derive(Debug, Clone)]
pub struct SingleThreadedCallback<T>
where
    T: Debug + Send + Sync,
{
    base: Arc<BaseCallback<T>>,
}

impl<T> SingleThreadedCallback<T>
where
    T: Debug + Send + Sync,
{
    /// Create a new single/current threaded callback holder.
    pub fn new() -> Self {
        Self {
            base: Arc::new(BaseCallback::<T>::new()),
        }
    }

    /// Invoke the currently registered callbacks and inform them of the given value.
    ///
    /// # Arguments
    ///
    /// * `value` - The value to invoke the callbacks with.
    pub fn invoke(&self, value: T) {
        self.base.invoke(value)
    }
}

impl<T> Callback<T> for SingleThreadedCallback<T>
where
    T: Debug + Send + Sync,
{
    fn subscribe(&self) -> Subscription<T> {
        self.base.subscribe()
    }

    fn subscribe_with(&self, subscriber: Subscriber<T>) {
        self.base.subscribe_with(subscriber)
    }
}

struct BaseCallback<T>
where
    T: Debug + Send + Sync,
{
    callbacks: Mutex<HashMap<CallbackHandle, UnboundedSender<Arc<T>>>>,
}

impl<T> BaseCallback<T>
where
    T: Debug + Send + Sync,
{
    fn new() -> Self {
        Self {
            callbacks: Mutex::new(HashMap::new()),
        }
    }

    fn subscribe(&self) -> Subscription<T> {
        let mut mutex = self.callbacks.lock().expect("failed to acquire lock");
        let (tx, rx) = tokio::sync::mpsc::unbounded_channel();
        let handle = CallbackHandle::new();
        mutex.insert(handle, tx);
        drop(mutex);
        trace!("Added callback {} to {:?}", handle, self);
        rx
    }

    fn subscribe_with(&self, subscriber: Subscriber<T>) {
        let mut mutex = self.callbacks.lock().expect("failed to acquire lock");
        let handle = CallbackHandle::new();
        mutex.insert(handle, subscriber);
        drop(mutex);
        trace!("Added callback {} to {:?}", handle, self);
    }

    fn invoke(&self, value: T) {
        let mut mutex = self.callbacks.lock().expect("failed to acquire lock");
        let value = Arc::new(value);

        trace!(
            "Invoking a total of {} callbacks for {:?}",
            mutex.len(),
            *value
        );

        let handles_to_remove: Vec<CallbackHandle> = mutex
            .iter()
            .map(|(handle, callback)| {
                BaseCallback::invoke_callback(handle, callback, value.clone())
            })
            .flat_map(|e| e)
            .collect();

        let total_handles = handles_to_remove.len();
        for handle in handles_to_remove {
            mutex.remove(&handle);
        }

        if total_handles > 0 {
            debug!("Removed a total of {} callbacks", total_handles);
        }
    }

    /// Try to invoke the callback for the given value.
    /// This is a convenience method for handling dropped callbacks.
    ///
    /// # Returns
    ///
    /// It returns the callback handle if the callback has been dropped.
    fn invoke_callback(
        handle: &CallbackHandle,
        callback: &UnboundedSender<Arc<T>>,
        value: Arc<T>,
    ) -> Option<CallbackHandle> {
        let start_time = Instant::now();
        if let Err(_) = callback.send(value) {
            trace!("Callback {} has been dropped", handle);
            return Some(handle.clone());
        }
        let elapsed = start_time.elapsed();
        let message = format!(
            "Callback {} took {}.{:03}ms to process the invocation",
            handle,
            elapsed.as_millis(),
            elapsed.subsec_micros() % 1000
        );
        if elapsed.as_millis() >= 1000 {
            warn!("{}", message);
        } else {
            trace!("{}", message);
        }

        None
    }
}

impl<T> Debug for BaseCallback<T>
where
    T: Debug + Send + Sync,
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("BaseCallback")
            .field("callbacks", &self.callbacks.lock().unwrap().len())
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::init_logger;
    use std::sync::mpsc::channel;
    use std::time::Duration;
    use tokio::{select, time};

    #[derive(Debug, Clone, PartialEq)]
    pub enum Event {
        Foo,
    }

    #[tokio::test]
    async fn test_multi_threaded_invoke() {
        init_logger!();
        let expected_result = Event::Foo;
        let (tx, mut rx) = tokio::sync::mpsc::channel(1);
        let callback = MultiThreadedCallback::<Event>::new();

        let mut receiver = callback.subscribe();
        tokio::spawn(async move {
            if let Some(e) = receiver.recv().await {
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

    #[test]
    fn test_multi_threaded_invoke_without_runtime() {
        init_logger!();
        let expected_result = Event::Foo;
        let (tx, rx) = channel();
        let runtime = Runtime::new().unwrap();
        let callback = MultiThreadedCallback::<Event>::new();

        let mut receiver = callback.subscribe();
        runtime.spawn(async move {
            if let Some(e) = receiver.recv().await {
                tx.send(e).unwrap();
            }
        });

        callback.invoke(expected_result.clone());
        let result = rx.recv_timeout(Duration::from_millis(50)).unwrap();

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
            if let Some(e) = receiver.recv().await {
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

    #[test]
    fn test_single_threaded_invoke() {
        init_logger!();
        let expected_result = Event::Foo;
        let runtime = Runtime::new().unwrap();
        let (tx, rx) = channel();
        let callback = SingleThreadedCallback::new();

        let mut receiver = callback.subscribe();
        runtime.spawn(async move {
            if let Some(e) = receiver.recv().await {
                tx.send(e).unwrap();
            }
        });

        callback.invoke(expected_result.clone());
        let result = rx.recv_timeout(Duration::from_millis(50)).unwrap();

        assert_eq!(expected_result, *result);
    }
}
