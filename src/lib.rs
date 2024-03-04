use std::sync::{Arc, RwLock};

/// Defines the `State` type as an atomically reference-counted read/write lock containing an optional value of type `T`.
/// This allows for the safe sharing and modification of state across threads.
///
/// # Example
///
/// ```
/// let state: State<i32> = Arc::new(RwLock::new(Some(42)));
/// ```
pub type State<T> = Arc<RwLock<Option<T>>>;

/// A type alias for a boxed dynamic closure that can modify the state.
///
/// This closure takes an `Option<T>` as its input and returns a `Result<(), Error>`
/// from the `error` module, allowing for error handling. It is both `Send` and `Sync`,
/// making it safe to be sent to or shared between different threads. This flexibility
/// makes it suitable for concurrent environments where state modifications need to be
/// synchronized across threads.
///
/// # Parameters
/// - `T`: The type of the value that the closure can accept. It uses `Option<T>` to
///   allow for the possibility of resetting or clearing the state by passing `None`.
///
/// # Returns
/// - `error::Result<()>`: A result indicating success (`Ok(())`) or containing an error
///   (`Err`) if the state modification fails.
///
/// # Traits
/// - `Send`: Allows the `StateSetter` to be transferred across thread boundaries.
/// - `Sync`: Allows the `StateSetter` to be accessed from multiple threads simultaneously.
///
/// # Examples
///
/// ```rust
///
/// pub type StateSetter<T> = Box<dyn Fn(Option<T>) -> error::Result<()> + Send + Sync>;
///
/// // Example usage of `StateSetter`
/// fn main() -> error::Result<()> {
///     // A sample state setter that simply prints the value or "Reset" if None
///     let setter: StateSetter<String> = Box::new(|opt| {
///         match opt {
///             Some(value) => println!("New value: {}", value),
///             None => println!("State reset"),
///         }
///         Ok(())
///     });
///
///     // Using the state setter
///     setter(Some("Hello, world!".to_string()))?;
///     setter(None)?;
///
///     Ok(())
/// }
/// ```
pub type StateSetter<T> = Box<dyn Fn(Option<T>) -> error::Result<()> + Send + Sync>;


/// Submodule defining possible errors.
pub mod error;

/// The `StateBuffer` trait defines the behavior of a state buffer.
/// In this context, it acts as a marker trait without methods.
pub trait StateBuffer{}

/// The `StateManager` trait provides functionality for creating new states.
/// It requires the state type `S` to be sendable across threads, synchronizable, have a default value,
/// be cloneable, and have a `'static` lifetime.
///
/// # Examples
///
/// ```
/// struct MyStateBuffer;
///
/// impl StateBuffer for MyStateBuffer {}
///
/// impl StateManager<String> for MyStateBuffer {
///     fn new_state(data: Option<String>) -> (State<String>, impl Fn(Option<String>) -> error::Result<()>) {
///         // Implementation of the method
///     }
/// }
/// ```
pub trait StateManager<S> 
    where 
    S: Send + Sync + Default + Clone + 'static,
{
    /// Creates a new state with initial data and returns a tuple containing `State<S>` and a function for modifying it.
    ///
    /// # Arguments
    ///
    /// * `data` - The initial state value of type `S`.
    ///
    /// # Return Value
    ///
    /// Returns a tuple of `State<S>` and a function for modifying the state.
    fn new_state(data: Option<S>) -> (State<S>, StateSetter<S>); 
}

/// Implement the `StateManager` trait for all types `T` that implement `StateBuffer`.
impl<T, S> StateManager<S> for T
    where 
    S: Send + Sync + Default + Clone + 'static,
    T: StateBuffer
{
    
    fn new_state(data: Option<S>) -> (
        State<S>, 
        StateSetter<S>
    ) {
        let (state, state_for_setter) = match data {
            Some(value) => {
                let state = Arc::new(RwLock::new(Some(value)));
                let state_for_setter = state.clone();
                (state, state_for_setter)
            },
            None => {
                let state = Arc::new(RwLock::new(None));
                let state_for_setter = state.clone(); 
                (state, state_for_setter)   
            },
        };
        let setter = move |data: Option<S>| -> error::Result<()> {
            let state_guard_result = state_for_setter.write();
            match state_guard_result {
                Ok(mut state_guard) => {
                    *state_guard = data;
                    Ok(())
                },
                Err(_) => Err(error::StateError::Default(String::from("Lock error"))),
            }

        };
        (state, Box::new(setter))
    }
}

/// The `Getter` trait provides a `get` method for retrieving the value from the state.
///
/// # Examples
///
/// ```
/// let state: State<i32> = Arc::new(RwLock::new(Some(42)));
/// assert_eq!(state.get(), Some(42));
/// ```
pub trait Getter<T> {
    /// Returns the current value of the state, if it exists.
    ///
    /// # Return Value
    ///
    /// Returns `Option<T>`, where `T` is the type of the value stored in the state.
    fn get(&self) -> Option<T>;
}

impl<T> Getter<T> for State<T>
where T: Clone
{
    fn get(&self) -> Option<T> {
        match self.read() {
            Ok(state_guard) => {
                let cl = state_guard.clone();
                cl
            },
            Err(_) => {
                None::<T>
            },
        }
    }
} 

#[cfg(test)]
pub mod tests {
    use std::{thread, time::Duration};

    use crate::{Getter, StateBuffer, StateManager};

    pub struct S;
    impl StateBuffer for S{}

    #[test]
    fn test_init_some_value(){
        let (state, _) = S::new_state(Some(42));
        assert_eq!(state.get(), Some(42));
    }

    #[test]
    fn test_init_none_value(){
        let (state, _) = S::new_state(None::<i32>);
        assert_eq!(state.get(), None);
    }

    #[test]
    fn test_set_new_value() {
        let (state, set_state) = S::new_state(Some(42));
        assert_eq!(state.get(), Some(42));
        // Доступ к данным имеет один поток => ошибки блокировки значения не может быть
        // Использование .unwrap() безопасно и оправдано в этом сценарии
        set_state(None).unwrap();
        assert_eq!(state.get(), None);
    }

    #[test]
    fn test_multiple_threads_reading() {
        // Инициализируем состояние с некоторым начальным значением
        let (state, _setter) = S::new_state(Some(42));

        // Создаем вектор для хранения дескрипторов потоков
        let mut handles = vec![];

        // Запускаем несколько потоков для чтения из состояния
        for _ in 0..10 {
            let state_clone = state.clone();
            let handle = thread::spawn(move || {
                // Здесь каждый поток пытается прочитать значение из состояния
                let read_result = state_clone.get();
                assert_eq!(read_result, Some(42));
            });
            handles.push(handle);
        }

        for handle in handles {
            handle.join().unwrap();
        }
    }

    #[test]
    fn test_write_blocks_read() {
        let (state, setter) = S::new_state(Some(0)); // Начальное значение 0
        let state_clone_for_readers = state.clone();
        
        // Запускаем поток для записи нового значения
        let writer_handle = std::thread::spawn(move || {
            setter(Some(42)).unwrap(); // Записываем значение 42
            thread::sleep(Duration::from_millis(1500)); // Имитация долгой записи
        });
    
        let mut reader_handles = vec![];
    
        for _ in 0..10 {
            let state_clone_for_reader = state_clone_for_readers.clone();
            let handle = std::thread::spawn(move || {
                let read_value = state_clone_for_reader.get();
                read_value
            });
            reader_handles.push(handle);
        }
            writer_handle.join().unwrap();
    
        for handle in reader_handles {
            let read_value = handle.join().unwrap();
            assert_eq!(read_value, Some(42));
        }
    }
    
}