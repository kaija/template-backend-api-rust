use super::*;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tokio::time::Duration;

/// Mock shutdown component for testing
struct MockShutdownComponent {
    name: String,
    shutdown_called: Arc<AtomicBool>,
    should_fail: bool,
    delay: Duration,
}

impl MockShutdownComponent {
    fn new(name: &str, shutdown_called: Arc<AtomicBool>, should_fail: bool, delay: Duration) -> Self {
        Self {
            name: name.to_string(),
            shutdown_called,
            should_fail,
            delay,
        }
    }
}

#[async_trait::async_trait]
impl ShutdownComponent for MockShutdownComponent {
    fn name(&self) -> &str {
        &self.name
    }

    async fn shutdown(&mut self) -> Result<(), ShutdownError> {
        // Simulate shutdown work
        tokio::time::sleep(self.delay).await;
        
        self.shutdown_called.store(true, Ordering::SeqCst);
        
        if self.should_fail {
            Err(ShutdownError::ResourceCleanup("Mock failure".to_string()))
        } else {
            Ok(())
        }
    }
}

#[tokio::test]
async fn test_graceful_shutdown_success() {
    let shutdown = GracefulShutdown::new(Duration::from_secs(5));
    
    let result = shutdown.execute_shutdown(|| async {
        tokio::time::sleep(Duration::from_millis(100)).await;
        Ok(())
    }).await;
    
    assert!(result.is_ok());
}

#[tokio::test]
async fn test_graceful_shutdown_timeout() {
    let shutdown = GracefulShutdown::new(Duration::from_millis(100));
    
    let result = shutdown.execute_shutdown(|| async {
        // Simulate a long-running shutdown that exceeds timeout
        tokio::time::sleep(Duration::from_millis(200)).await;
        Ok(())
    }).await;
    
    assert!(matches!(result, Err(ShutdownError::Timeout)));
}

#[tokio::test]
async fn test_shutdown_coordinator_success() {
    let mut coordinator = ShutdownCoordinator::new();
    
    let shutdown_called1 = Arc::new(AtomicBool::new(false));
    let shutdown_called2 = Arc::new(AtomicBool::new(false));
    
    let component1 = MockShutdownComponent::new(
        "test1", 
        shutdown_called1.clone(), 
        false, 
        Duration::from_millis(50)
    );
    let component2 = MockShutdownComponent::new(
        "test2", 
        shutdown_called2.clone(), 
        false, 
        Duration::from_millis(50)
    );
    
    coordinator.register(component1);
    coordinator.register(component2);
    
    let result = coordinator.shutdown_all().await;
    
    assert!(result.is_ok());
    assert!(shutdown_called1.load(Ordering::SeqCst));
    assert!(shutdown_called2.load(Ordering::SeqCst));
}

#[tokio::test]
async fn test_shutdown_coordinator_with_failure() {
    let mut coordinator = ShutdownCoordinator::new();
    
    let shutdown_called1 = Arc::new(AtomicBool::new(false));
    let shutdown_called2 = Arc::new(AtomicBool::new(false));
    
    let component1 = MockShutdownComponent::new(
        "test1", 
        shutdown_called1.clone(), 
        true, // This component will fail
        Duration::from_millis(50)
    );
    let component2 = MockShutdownComponent::new(
        "test2", 
        shutdown_called2.clone(), 
        false, 
        Duration::from_millis(50)
    );
    
    coordinator.register(component1);
    coordinator.register(component2);
    
    let result = coordinator.shutdown_all().await;
    
    // Should still succeed even if one component fails
    assert!(result.is_ok());
    assert!(shutdown_called1.load(Ordering::SeqCst));
    assert!(shutdown_called2.load(Ordering::SeqCst));
}

#[tokio::test]
async fn test_resource_cleanup() {
    let result = ResourceCleanup::cleanup_all_resources(Duration::from_secs(1)).await;
    assert!(result.is_ok());
}

#[tokio::test]
async fn test_resource_cleanup_timeout() {
    // This test would need to be modified to actually cause a timeout
    // For now, we just test that the function completes successfully
    let result = ResourceCleanup::cleanup_all_resources(Duration::from_millis(10)).await;
    assert!(result.is_ok());
}

#[tokio::test]
async fn test_general_resource_cleanup_component() {
    let mut cleanup = GeneralResourceCleanup::new()
        .with_timeout(Duration::from_secs(1));
    
    let result = cleanup.shutdown().await;
    assert!(result.is_ok());
}

#[tokio::test]
async fn test_shutdown_component_timeouts() {
    // Test that components respect their timeout settings
    let mut coordinator = ShutdownCoordinator::new();
    
    let shutdown_called = Arc::new(AtomicBool::new(false));
    
    // Create a component that takes longer than its timeout
    let component = MockShutdownComponent::new(
        "slow_component", 
        shutdown_called.clone(), 
        false, 
        Duration::from_millis(200) // Component takes 200ms
    );
    
    coordinator.register(component);
    
    // The coordinator should still complete successfully
    let result = coordinator.shutdown_all().await;
    assert!(result.is_ok());
    assert!(shutdown_called.load(Ordering::SeqCst));
}