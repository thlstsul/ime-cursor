use anyhow::{Result, anyhow, bail};
use std::collections::VecDeque;
use std::sync::{Arc, Condvar, LockResult, Mutex};
use std::thread;
use std::time::{Duration, Instant};

// 事件结构体，包含数据和发送时间戳
struct Event<T> {
    data: T,
    scheduled_time: Instant,
}

// 发送端
pub struct Sender<T> {
    inner: Arc<Inner<T>>,
}

// 接收端
pub struct Receiver<T> {
    inner: Arc<Inner<T>>,
}

// 内部共享状态
struct Inner<T> {
    queue: Mutex<VecDeque<Event<T>>>,       // 事件队列
    current_event: Mutex<Option<Event<T>>>, // 当前待发送事件（可覆盖）
    condvar: Condvar,                       // 条件变量，用于线程通知
    delay: Duration,                        // 发送延迟
    active: Mutex<bool>,                    // 通道是否活跃
}

impl<T> Sender<T> {
    /// 发送事件，如果已有事件在等待，则覆盖它
    pub fn send(&self, data: T) -> Result<()> {
        let mut current_event = self.inner.current_event.lock().map_lock_err()?;

        if !*self.inner.active.lock().map_lock_err()? {
            bail!("Channel is closed");
        }

        // 创建新事件，安排在延迟后发送
        let scheduled_time = Instant::now() + self.inner.delay;
        *current_event = Some(Event {
            data,
            scheduled_time,
        });

        // 通知工作线程检查新事件
        self.inner.condvar.notify_one();
        Ok(())
    }

    /// 立即发送事件，不经过延迟
    #[allow(dead_code)]
    pub fn send_immediate(&self, data: T) -> Result<()> {
        let mut queue = self.inner.queue.lock().map_lock_err()?;

        if !*self.inner.active.lock().map_lock_err()? {
            bail!("Channel is closed");
        }

        // 清除当前待发送事件
        *self.inner.current_event.lock().map_lock_err()? = None;

        // 立即加入队列
        queue.push_back(Event {
            data,
            scheduled_time: Instant::now(),
        });

        // 通知接收端
        self.inner.condvar.notify_one();
        Ok(())
    }
}

impl<T> Receiver<T> {
    /// 接收事件，阻塞直到有事件可用或通道关闭
    pub fn recv(&self) -> Result<T> {
        let mut queue = self.inner.queue.lock().map_lock_err()?;

        // 等待队列中有事件或通道关闭
        while queue.is_empty() {
            if !*self.inner.active.lock().map_lock_err()? {
                bail!("Channel is closed");
            }
            queue = self.inner.condvar.wait(queue).map_lock_err()?;
        }

        if let Some(event) = queue.pop_front() {
            Ok(event.data)
        } else {
            bail!("Channel is empty")
        }
    }

    /// 尝试接收事件，立即返回
    #[allow(dead_code)]
    pub fn try_recv(&self) -> Result<T> {
        let mut queue = self.inner.queue.lock().map_lock_err()?;

        if let Some(event) = queue.pop_front() {
            Ok(event.data)
        } else if !*self.inner.active.lock().map_lock_err()? {
            bail!("Channel is closed")
        } else {
            bail!("Channel is empty")
        }
    }
}

/// 创建新的延迟通道
pub fn channel<T: Send + 'static>(delay: Duration) -> (Sender<T>, Receiver<T>) {
    let inner = Arc::new(Inner {
        queue: Mutex::new(VecDeque::new()),
        current_event: Mutex::new(None),
        condvar: Condvar::new(),
        delay,
        active: Mutex::new(true),
    });

    let sender = Sender {
        inner: inner.clone(),
    };

    let receiver = Receiver {
        inner: inner.clone(),
    };

    // 启动工作线程来处理延迟发送
    thread::spawn(move || {
        worker_thread(inner);
    });

    (sender, receiver)
}

/// 工作线程，负责处理延迟发送和事件覆盖
fn worker_thread<T: Send + 'static>(inner: Arc<Inner<T>>) {
    loop {
        let sleep_duration = {
            let current_event = inner.current_event.lock().unwrap();

            // 如果没有待发送事件，等待通知
            if let Some(event) = current_event.as_ref() {
                // 计算需要等待的时间
                let now = Instant::now();
                if event.scheduled_time > now {
                    Some(event.scheduled_time - now)
                } else {
                    Some(Duration::from_secs(0)) // 立即发送
                }
            } else {
                None // 无事件，等待通知
            }
        };

        let mut current_event = match sleep_duration {
            Some(duration) => {
                // 锁定当前事件，准备等待
                let current_event = inner.current_event.lock().unwrap();

                if current_event.is_none() {
                    // 在等待期间事件被清除了，重新开始循环
                    continue;
                }

                // 等待指定时间或收到新事件通知
                let (new_guard, _) = inner.condvar.wait_timeout(current_event, duration).unwrap();
                new_guard
            }
            None => {
                // 无事件，等待通知
                let current_event = inner.current_event.lock().unwrap();
                inner.condvar.wait(current_event).unwrap()
            }
        };

        // 检查通道是否已关闭
        if !*inner.active.lock().unwrap() {
            break;
        }

        // 如果事件仍然存在且已到发送时间，发送它
        if let Some(event) = current_event.take() {
            if event.scheduled_time <= Instant::now() {
                let mut queue = inner.queue.lock().unwrap();
                queue.push_back(event);
                inner.condvar.notify_one(); // 通知接收端
            } else {
                // 还没到时间，放回去
                *current_event = Some(event);
            }
        }
    }
}

impl<T> Clone for Sender<T> {
    fn clone(&self) -> Sender<T> {
        Sender {
            inner: self.inner.clone(),
        }
    }
}

impl<T> Drop for Inner<T> {
    fn drop(&mut self) {
        *self.active.lock().unwrap() = false;
        self.condvar.notify_all(); // 唤醒所有等待的线程
    }
}

impl<T> Drop for Sender<T> {
    fn drop(&mut self) {
        // 发送端被丢弃时，关闭通道
        *self.inner.active.lock().unwrap() = false;
        self.inner.condvar.notify_all();
    }
}

impl<T> Drop for Receiver<T> {
    fn drop(&mut self) {
        // 接收端被丢弃时，关闭通道
        *self.inner.active.lock().unwrap() = false;
        self.inner.condvar.notify_all();
    }
}

trait MapMutexLockError<T> {
    fn map_lock_err(self) -> Result<T>;
}

impl<T> MapMutexLockError<T> for LockResult<T> {
    fn map_lock_err(self) -> Result<T> {
        self.map_err(|e| anyhow!("{}", e))
    }
}

// 测试代码
#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::mpsc;

    #[test]
    fn test_delayed_send() {
        let delay = Duration::from_millis(100);
        let (tx, rx) = channel(delay);

        let start = Instant::now();

        // 发送事件
        tx.send("event1").unwrap();
        thread::sleep(Duration::from_millis(50));

        // 在延迟时间内发送新事件，覆盖前一个
        tx.send("event2").unwrap();
        thread::sleep(Duration::from_millis(50));

        // 接收事件
        let result = rx.recv().unwrap();

        // 验证收到了第二个事件
        assert_eq!(result, "event2");

        // 验证延迟生效
        let elapsed = start.elapsed();
        assert!(elapsed >= Duration::from_millis(150));
        assert!(elapsed < Duration::from_millis(160));
    }

    #[test]
    fn test_immediate_send() {
        let delay = Duration::from_millis(100);
        let (tx, rx) = channel(delay);

        let start = Instant::now();

        // 立即发送
        tx.send_immediate("immediate").unwrap();

        // 接收事件
        let result = rx.recv().unwrap();

        // 验证立即收到
        assert_eq!(result, "immediate");

        // 验证没有延迟
        let elapsed = start.elapsed();
        assert!(elapsed < Duration::from_millis(50));
    }

    #[test]
    fn test_multiple_events() {
        let delay = Duration::from_millis(50);
        let (tx, rx) = channel(delay);

        let (test_tx, test_rx) = mpsc::channel();

        // 启动接收线程
        thread::spawn(move || {
            let results: Vec<String> = (0..3).map(|_| rx.recv().unwrap()).collect();
            test_tx.send(results).unwrap();
        });

        // 发送多个事件，只有最后一个应该被保留
        tx.send("event1".to_string()).unwrap();
        thread::sleep(Duration::from_millis(10));
        tx.send("event2".to_string()).unwrap();
        thread::sleep(Duration::from_millis(10));
        tx.send("event3".to_string()).unwrap();

        // 等待足够长时间让所有事件处理
        thread::sleep(Duration::from_millis(200));

        // 发送更多事件
        tx.send("event4".to_string()).unwrap();
        thread::sleep(Duration::from_millis(60));

        tx.send("event5".to_string()).unwrap();
        thread::sleep(Duration::from_millis(60));

        // 关闭发送端
        drop(tx);

        let results = test_rx.recv().unwrap();

        // 应该只收到event3, event4, event5
        assert_eq!(results, vec!["event3", "event4", "event5"]);
    }
}
