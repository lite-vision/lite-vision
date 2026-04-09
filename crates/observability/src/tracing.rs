use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::sync::RwLock;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Hash)]
pub struct TraceId(pub [u8; 32]);

impl Default for TraceId {
    fn default() -> Self {
        Self([0u8; 32])
    }
}

impl TraceId {
    pub fn new() -> Self {
        use blake3::Hasher;
        let mut hasher = Hasher::new();
        hasher.update(&std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
            .to_le_bytes());
        hasher.update(&rand::random::<[u8; 32]>());
        Self(*hasher.finalize().as_bytes())
    }

    pub fn from_bytes(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }

    pub fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }

    pub fn to_hex(&self) -> String {
        hex::encode(self.0)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Hash)]
pub struct SpanId(pub u64);

impl Default for SpanId {
    fn default() -> Self {
        Self(0)
    }
}

impl SpanId {
    pub fn new() -> Self {
        Self(rand::random())
    }

    pub fn from_u64(id: u64) -> Self {
        Self(id)
    }

    pub fn as_u64(&self) -> u64 {
        self.0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct TraceContext {
    pub trace_id: TraceId,
    pub span_id: SpanId,
    pub parent_span_id: Option<SpanId>,
}

impl TraceContext {
    pub fn new(trace_id: TraceId) -> Self {
        let span_id = SpanId::new();
        Self {
            trace_id,
            span_id,
            parent_span_id: None,
        }
    }

    pub fn new_child(parent: &TraceContext) -> Self {
        let span_id = SpanId::new();
        Self {
            trace_id: parent.trace_id,
            span_id,
            parent_span_id: Some(parent.span_id),
        }
    }

    pub fn with_span_id(mut self, span_id: SpanId) -> Self {
        self.span_id = span_id;
        self
    }

    pub fn with_parent(mut self, parent_span_id: SpanId) -> Self {
        self.parent_span_id = Some(parent_span_id);
        self
    }

    pub fn to_json(&self) -> String {
        serde_json::to_string(self).unwrap_or_default()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Span {
    pub trace_context: TraceContext,
    pub operation_name: String,
    pub start_timestamp_ms: u64,
    pub end_timestamp_ms: Option<u64>,
    pub tags: std::collections::HashMap<String, String>,
    pub logs: Vec<SpanLog>,
}

impl Span {
    pub fn new(operation_name: &str, context: TraceContext) -> Self {
        Self {
            trace_context: context,
            operation_name: operation_name.to_string(),
            start_timestamp_ms: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_millis() as u64,
            end_timestamp_ms: None,
            tags: std::collections::HashMap::new(),
            logs: Vec::new(),
        }
    }

    pub fn finish(&mut self) {
        self.end_timestamp_ms = Some(
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_millis() as u64
        );
    }

    pub fn duration_ms(&self) -> Option<u64> {
        self.end_timestamp_ms.map(|end| end - self.start_timestamp_ms)
    }

    pub fn with_tag(mut self, key: &str, value: &str) -> Self {
        self.tags.insert(key.to_string(), value.to_string());
        self
    }

    pub fn add_log(&mut self, message: &str) {
        self.logs.push(SpanLog {
            timestamp_ms: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_millis() as u64,
            message: message.to_string(),
        });
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpanLog {
    pub timestamp_ms: u64,
    pub message: String,
}

pub struct Tracer {
    current_context: Arc<RwLock<Option<TraceContext>>>,
}

impl Tracer {
    pub fn new() -> Self {
        Self {
            current_context: Arc::new(RwLock::new(None)),
        }
    }

    pub fn arc(&self) -> Arc<RwLock<Option<TraceContext>>> {
        self.current_context.clone()
    }

    pub async fn start_trace(&self, operation_name: &str) -> Span {
        let context = TraceContext::new(TraceId::new());
        *self.current_context.write().await = Some(context);
        Span::new(operation_name, context)
    }

    pub async fn start_child_span(&self, operation_name: &str) -> Option<Span> {
        let context = self.current_context.read().await;
        if let Some(parent) = context.as_ref() {
            let child = TraceContext::new_child(parent);
            *self.current_context.write().await = Some(child);
            Some(Span::new(operation_name, child))
        } else {
            None
        }
    }

    pub async fn get_current_context(&self) -> Option<TraceContext> {
        self.current_context.read().await.clone()
    }

    pub async fn set_context(&self, context: TraceContext) {
        *self.current_context.write().await = Some(context);
    }

    pub async fn clear_context(&self) {
        *self.current_context.write().await = None;
    }

    pub fn with_context<F, R>(&self, context: TraceContext, f: F) -> R
    where
        F: FnOnce() -> R,
    {
        let mut guard = self.current_context.blocking_write();
        let old_context = guard.replace(context);
        let result = f();
        *guard = old_context;
        result
    }
}

impl Default for Tracer {
    fn default() -> Self {
        Self::new()
    }
}

pub fn propagate_trace_context(headers: &mut std::collections::HashMap<String, String>, context: &TraceContext) {
    headers.insert("x-trace-id".to_string(), context.trace_id.to_hex());
    headers.insert("x-span-id".to_string(), context.span_id.as_u64().to_string());
    if let Some(parent) = context.parent_span_id {
        headers.insert("x-parent-span-id".to_string(), parent.as_u64().to_string());
    }
}

pub fn extract_trace_context(headers: &std::collections::HashMap<String, String>) -> Option<TraceContext> {
    let trace_id = headers.get("x-trace-id").and_then(|v| {
        let bytes = hex::decode(v).ok()?;
        let mut arr = [0u8; 32];
        arr.copy_from_slice(&bytes[..32.min(bytes.len())]);
        Some(TraceId(arr))
    })?;

    let span_id = headers.get("x-span-id").and_then(|v| {
        v.parse::<u64>().ok().map(SpanId)
    })?;

    let parent_span_id = headers.get("x-parent-span-id").and_then(|v| {
        v.parse::<u64>().ok().map(SpanId)
    });

    Some(TraceContext {
        trace_id,
        span_id,
        parent_span_id,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_trace_id_generation() {
        let id1 = TraceId::new();
        let id2 = TraceId::new();
        assert_ne!(id1, id2);
    }

    #[test]
    fn test_trace_context_creation() {
        let ctx = TraceContext::new(TraceId::new());
        assert_ne!(ctx.span_id, SpanId::default());
        assert_eq!(ctx.parent_span_id, None);
    }

    #[test]
    fn test_child_span_creation() {
        let parent = TraceContext::new(TraceId::new());
        let child = TraceContext::new_child(&parent);
        
        assert_eq!(child.trace_id, parent.trace_id);
        assert_ne!(child.span_id, parent.span_id);
        assert_eq!(child.parent_span_id, Some(parent.span_id));
    }

    #[test]
    fn test_span_lifecycle() {
        let ctx = TraceContext::new(TraceId::new());
        let mut span = Span::new("test_operation", ctx);
        
        assert_eq!(span.operation_name, "test_operation");
        assert!(span.end_timestamp_ms.is_none());
        
        span.finish();
        assert!(span.end_timestamp_ms.is_some());
        assert!(span.duration_ms().is_some());
    }

    #[tokio::test]
    async fn test_tracer_context_management() {
        let tracer = Tracer::new();
        
        assert!(tracer.get_current_context().await.is_none());
        
        let span = tracer.start_trace("test").await;
        let ctx = tracer.get_current_context().await;
        assert!(ctx.is_some());
        
        tracer.clear_context().await;
        assert!(tracer.get_current_context().await.is_none());
    }

    #[test]
    fn test_trace_context_propagation() {
        let mut headers = std::collections::HashMap::new();
        let ctx = TraceContext::new(TraceId::new());
        
        propagate_trace_context(&mut headers, &ctx);
        
        assert!(headers.contains_key("x-trace-id"));
        assert!(headers.contains_key("x-span-id"));
        
        let extracted = extract_trace_context(&headers);
        assert!(extracted.is_some());
    }
}
