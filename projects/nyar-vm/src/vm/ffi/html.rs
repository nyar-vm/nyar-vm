use crate::vm::core::NyarVM;
use crate::vm::value::Value;
use crate::vm::ffi::{FFIFunction, FFIResult, FFISignature, FFIType};
use nyar_types::NyarError;
use nipper::Document;
use std::sync::{Arc, RwLock, OnceLock};
use std::collections::HashMap;

static DOCUMENTS: OnceLock<RwLock<HashMap<usize, Document>>> = OnceLock::new();
static SELECTIONS: OnceLock<RwLock<HashMap<usize, Vec<String>>>> = OnceLock::new();
static NEXT_ID: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(1);

fn get_documents() -> &'static RwLock<HashMap<usize, Document>> {
    DOCUMENTS.get_or_init(|| RwLock::new(HashMap::new()))
}

fn get_selections() -> &'static RwLock<HashMap<usize, Vec<String>>> {
    SELECTIONS.get_or_init(|| RwLock::new(HashMap::new()))
}

pub struct StdHtmlParse;
impl FFIFunction for StdHtmlParse {
    fn signature(&self) -> Option<FFISignature> {
        Some(FFISignature {
            params: vec![FFIType::String],
            ret: FFIType::Int,
        })
    }
    fn call(&self, _vm: &mut NyarVM, args: Vec<Value>) -> FFIResult {
        let html = args.get(0).ok_or_else(|| NyarError::RuntimeError("Missing html argument".to_string()))?;
        let html_str = html.try_as_str().ok_or_else(|| NyarError::RuntimeError("Html must be a string".to_string()))?;
        
        let doc = Document::from(html_str);
        let id = NEXT_ID.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        
        get_documents().write().unwrap().insert(id, doc);
        
        Ok(Value::int(id as i64))
    }
}

pub struct StdHtmlSelectText;
impl FFIFunction for StdHtmlSelectText {
    fn signature(&self) -> Option<FFISignature> {
        Some(FFISignature {
            params: vec![FFIType::Int, FFIType::String],
            ret: FFIType::List,
        })
    }
    fn call(&self, vm: &mut NyarVM, args: Vec<Value>) -> FFIResult {
        let id = args.get(0).ok_or_else(|| NyarError::RuntimeError("Missing id argument".to_string()))?.as_int() as usize;
        let selector = args.get(1).ok_or_else(|| NyarError::RuntimeError("Missing selector argument".to_string()))?
            .try_as_str().ok_or_else(|| NyarError::RuntimeError("Selector must be a string".to_string()))?;
            
        let docs = get_documents().read().unwrap();
        let doc = docs.get(&id).ok_or_else(|| NyarError::RuntimeError(format!("Document not found: {}", id)))?;
        
        let texts: Vec<Value> = doc.select(selector).iter()
            .map(|s| Value::string(s.text().to_string(), &vm.gc))
            .collect();
            
        Ok(Value::list(texts, &vm.gc))
    }
}

pub struct StdHtmlSelectAttr;
impl FFIFunction for StdHtmlSelectAttr {
    fn signature(&self) -> Option<FFISignature> {
        Some(FFISignature {
            params: vec![FFIType::Int, FFIType::String, FFIType::String],
            ret: FFIType::List,
        })
    }
    fn call(&self, vm: &mut NyarVM, args: Vec<Value>) -> FFIResult {
        let id = args.get(0).ok_or_else(|| NyarError::RuntimeError("Missing id argument".to_string()))?.as_int() as usize;
        let selector = args.get(1).ok_or_else(|| NyarError::RuntimeError("Missing selector argument".to_string()))?
            .try_as_str().ok_or_else(|| NyarError::RuntimeError("Selector must be a string".to_string()))?;
        let attr = args.get(2).ok_or_else(|| NyarError::RuntimeError("Missing attr argument".to_string()))?
            .try_as_str().ok_or_else(|| NyarError::RuntimeError("Attr must be a string".to_string()))?;
            
        let docs = get_documents().read().unwrap();
        let doc = docs.get(&id).ok_or_else(|| NyarError::RuntimeError(format!("Document not found: {}", id)))?;
        
        let attrs: Vec<Value> = doc.select(selector).iter()
            .filter_map(|s| s.attr(attr).map(|a| Value::string(a.to_string(), &vm.gc)))
            .collect();
            
        Ok(Value::list(attrs, &vm.gc))
    }
}
