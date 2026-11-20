/// Ressources statiques jamais demandées pendant un crawl (règle D8).
pub const ASSET_IGNORE_REGEX: &str =
    r"(?i)\.(js|mjs|css|woff2?|ttf|otf|eot|pdf|zip|rar|7z|docx?|xlsx?|pptx?|mp3|mp4|webm)(\?|#|$)";
