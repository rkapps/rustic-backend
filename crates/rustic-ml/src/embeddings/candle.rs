use anyhow::Result;
use candle_core::{DType, Device, Tensor};
use candle_transformers::models::{
    bert::{BertModel, Config},
    mimi::candle_nn::VarBuilder,
};
use std::{path::PathBuf, sync::Arc};
use tokenizers::Tokenizer;
pub struct CandleEmbeddingClient {
    pub model: Arc<BertModel>,
    pub tokenizer: Arc<Tokenizer>,
    pub device: Device,
}

impl CandleEmbeddingClient {
    pub async fn new(cache_path: &str) -> Result<Self> {
        let cache = PathBuf::from(cache_path);

        let model_file = cache.join("model.safetensors");
        let config_file = cache.join("config.json");
        let tokenizer_file = cache.join("tokenizer.json");

        println!("model:     {:?}", model_file);
        println!("config:    {:?}", config_file);
        println!("tokenizer: {:?}", tokenizer_file);

        let device = Device::Cpu;
        let tokenizer = Tokenizer::from_file(tokenizer_file).map_err(anyhow::Error::msg)?;

        let config: Config = serde_json::from_str(&std::fs::read_to_string(config_file)?)?;

        let vb =
            unsafe { VarBuilder::from_mmaped_safetensors(&[model_file], DType::F32, &device)? };

        let model = BertModel::load(vb, &config)?;
        Ok(Self {
            model: Arc::new(model),
            tokenizer: Arc::new(tokenizer),
            device,
        })
    }

    async fn embed_text(&self, text: &str) -> Result<Vec<f32>> {
        let model = self.model.clone();
        let tokenizer = self.tokenizer.clone();
        // let text = text.to_string();
        // Make sure you're using the prefix
        let text = format!("passage: {}", text);
        let device = self.device.clone();

        // Run on blocking thread pool — don't block async executor
        tokio::task::spawn_blocking(move || {
            let tokens = tokenizer.encode(text, true).map_err(anyhow::Error::msg)?;

            let token_ids = Tensor::new(tokens.get_ids(), &device)?.unsqueeze(0)?;

            let token_type_ids = token_ids.zeros_like()?;

            let output = model.forward(&token_ids, &token_type_ids, None)?;

            // Mean pooling
            let embedding = (output.sum(1)? / output.dim(1)? as f64)?;
            let embedding = embedding.squeeze(0)?;

            let norm = (embedding.sqr()?.sum(0)? + 1e-12)?.sqrt()?;
            let normalized = embedding.broadcast_div(&norm)?;

            Ok(normalized.to_vec1::<f32>()?)
        })
        .await?
    }
}

fn cosine_similarity(a: &[f32], b: &[f32]) -> f32 {
    let dot: f32 = a.iter().zip(b.iter()).map(|(x, y)| x * y).sum();
    let norm_a: f32 = a.iter().map(|x| x * x).sum::<f32>().sqrt();
    let norm_b: f32 = b.iter().map(|x| x * x).sum::<f32>().sqrt();
    dot / (norm_a * norm_b)
}

#[cfg(test)]
mod tests {

    use super::*;

    #[tokio::test]
    async fn test_candle_embedding_client() -> Result<()> {
        let path = "/media/raghu/data2/Workspace/AppData/hf-home/minilm";
        let client = CandleEmbeddingClient::new(path).await?;
        // let embedding = client.embed_text("AbbVie Inc. (AbbVie) is a research-based biopharmaceutical company. The Company is engaged in the discovery development manufacture and sale of a range of pharmaceutical products. Its products are focused on treating conditions such as chronic autoimmune diseases in rheumatology gastroenterology and dermatology oncology including blood cancers virology including hepatitis C virus (HCV) and human immunodeficiency virus (HIV) neurological disorders such as Parkinson's disease and multiple sclerosis metabolic diseases including thyroid disease and complications associated with cystic fibrosis and other serious health conditions. It offers products in various categories including HUMIRA (adalimumab) Oncology products Virology Products Additional Virology products Metabolics/Hormones products Endocrinology products and other products which include Duopa and Duodopa (carbidopa and levodopa) Anesthesia products and ZINBRYTA (daclizumab).").await?;
        // println!("Embedding: {:?}", embedding.len());

        // // Test 1 — similar pharma companies
        // let abbvie = client.embed_text("passage: AbbVie Inc is a biopharmaceutical company...").await?;
        // let pfizer = client.embed_text("passage: Pfizer Inc is a biopharmaceutical company focused on medicines vaccines...").await?;

        // // Test 2 — unrelated
        // let nvidia = client.embed_text("passage: NVIDIA Corporation designs graphics processing units for gaming and AI...").await?;

        // println!("AbbVie vs Pfizer:  {:.4}", cosine_similarity(abbvie.as_slice(), pfizer.as_slice()));
        // println!("AbbVie vs NVIDIA:  {:.4}", cosine_similarity(abbvie.as_slice(), nvidia.as_slice()));

        // let nvidia = client.embed_text("passage: NVIDIA Corporation designs graphics processing units GPU semiconductor AI chips data center").await?;
        // let amd = client
        //     .embed_text(
        //         "passage: AMD Advanced Micro Devices semiconductor CPU GPU processors computing",
        //     )
        //     .await?;
        // let abbvie = client
        //     .embed_text("passage: AbbVie biopharmaceutical HUMIRA autoimmune rheumatology oncology")
        //     .await?;

        // println!("NVIDIA vs AMD:    {:.4}", cosine_similarity(&nvidia, &amd));
        // println!(
        //     "NVIDIA vs AbbVie: {:.4}",
        //     cosine_similarity(&nvidia, &abbvie)
        // );

        let start = std::time::Instant::now();
        let query   = client.embed_text("find healthcare stocks").await?;
        println!("time to embed query: {:?} vector: {}", start.elapsed(), query.len());        

let long_text = "AbbVie Inc. (AbbVie) is a research-based biopharmaceutical company. The Company is engaged in the discovery development manufacture and sale of a range of pharmaceutical products. Its products are focused on treating conditions such as chronic autoimmune diseases in rheumatology gastroenterology and dermatology oncology including blood cancers virology including hepatitis C virus (HCV) and human immunodeficiency virus (HIV) neurological disorders such as Parkinson's disease and multiple sclerosis metabolic diseases including thyroid disease and complications associated with cystic fibrosis and other serious health conditions. It offers products in various categories including HUMIRA (adalimumab) Oncology products Virology Products Additional Virology products Metabolics/Hormones products Endocrinology products and other products which include Duopa and Duodopa (carbidopa and levodopa) Anesthesia products and ZINBRYTA (daclizumab).";
let short_abbvie = "AbbVie biopharmaceutical HUMIRA autoimmune rheumatology oncology blood cancers HIV hepatitis Parkinson's";

let article2 = client.embed_text(short_abbvie).await?;

println!("time to embed shorter text: {:?}", start.elapsed());

let start = std::time::Instant::now();
println!("query vs article2: {:.4}", cosine_similarity(&query, &article2));

// println!("query vs abbvie article: {:.4}", cosine_similarity(&query, &article2));
println!("time for cosine_similarity {:?}", start.elapsed());

        Ok(())
    }
}
