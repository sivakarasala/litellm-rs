mod health_check;

pub use health_check::health_check;

use utoipa::openapi::security::{HttpAuthScheme, HttpBuilder, SecurityScheme};
use utoipa::{Modify, OpenApi};

use crate::proxy::types::{
    ChatChoice, ChatCompletionRequest, ChatCompletionResponse, ChatMessage, CompletionRequest,
    EmbeddingData, EmbeddingRequest, EmbeddingResponse, ModelsResponse, OpenAIError,
    OpenAIErrorDetail, Usage,
};

struct BearerAuth;
impl Modify for BearerAuth {
    fn modify(&self, openapi: &mut utoipa::openapi::OpenApi) {
        if let Some(components) = openapi.components.as_mut() {
            components.add_security_scheme(
                "bearer_token",
                SecurityScheme::Http(
                    HttpBuilder::new()
                        .scheme(HttpAuthScheme::Bearer)
                        .bearer_format("litellm-rs virtual key")
                        .build(),
                ),
            );
        }
    }
}

#[derive(OpenApi)]
#[openapi(
    paths(
        health_check::health_check,
        crate::proxy::chat_completions::chat_completions,
        crate::proxy::completions::completions,
        crate::proxy::embeddings::embeddings,
        crate::proxy::models::list_models,
    ),
    components(schemas(
        ChatCompletionRequest,
        ChatCompletionResponse,
        ChatMessage,
        ChatChoice,
        CompletionRequest,
        EmbeddingRequest,
        EmbeddingResponse,
        EmbeddingData,
        ModelsResponse,
        Usage,
        OpenAIError,
        OpenAIErrorDetail,
    )),
    modifiers(&BearerAuth),
    tags(
        (name = "proxy", description = "OpenAI-compatible proxy endpoints"),
        (name = "v1", description = "Admin API endpoints"),
    )
)]
pub struct ApiDoc;
