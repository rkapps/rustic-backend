#!/bin/bash
# deploy-api.sh
ORG_ID="1032448450426"
PROJECT_ID="rustic-ai-rkapps"
RUSTIC_AI_PROJECT_ID=$PROJECT_ID
REGION="us-central1"
IMAGE_REGISTRY="us-central1-docker.pkg.dev/$PROJECT_ID"
COMPUTE_SA_NUMBER=$(gcloud projects describe $PROJECT_ID --format="value(projectNumber)")
COMPUTE_SA="$COMPUTE_SA_NUMBER-compute@developer.gserviceaccount.com"
GCS_BUCKET="$PROJECT_ID-data"

RUSTIC_AI_CONFIG_PATH="gs://$GCS_BUCKET/config"
RUSTIC_AI_BSET_DATA_PATH="gs://$GCS_BUCKET/bset"
FINTRACKER_DB_NAME="finTracker"
RUSTIC_AI_DB_NAME="rusticai"
RUST_LOG_VALUE="rustic_ai_api=info,rustic_boot=info,rustic_agent=info"


#docker build --no-cache -f Dockerfile.api \
docker build -f Dockerfile.api \
  -t $IMAGE_REGISTRY/rustic-ai-api/rustic-ai-api . \
  --build-arg GITHUB_USERNAME=$GITHUB_USERNAME \
  --build-arg GITHUB_TOKEN=$GITHUB_TOKEN \
  && docker push $IMAGE_REGISTRY/rustic-ai-api/rustic-ai-api \
  && gcloud run deploy rustic-ai-api \
        --image $IMAGE_REGISTRY/rustic-ai-api/rustic-ai-api \
        --region us-central1 \
        --allow-unauthenticated \
        --set-env-vars RUSTIC_AI_CONFIG_PATH=$RUSTIC_AI_CONFIG_PATH \
        --set-env-vars RUSTIC_AI_BSET_DATA_PATH=$RUSTIC_AI_BSET_DATA_PATH \
        --set-env-vars RUSTIC_AI_DB_NAME=$RUSTIC_AI_DB_NAME \
        --set-env-vars RUSTIC_AI_PROJECT_ID=$PROJECT_ID \
        --set-env-vars FINTRACKER_DB_NAME=$FINTRACKER_DB_NAME \
        --set-env-vars MONGO_URI=$MONGO_URI \
        --set-env-vars GCP_LLM_BASE_URL=$GCP_LLM_BASE_URL \
        --set-env-vars OPENAI_API_KEY=$OPENAI_API_KEY \
        --set-env-vars GEMINI_API_KEY=$GEMINI_API_KEY \
        --set-env-vars ANTHROPIC_API_KEY=$ANTHROPIC_API_KEY \
        --set-env-vars FRED_API_KEY=$FRED_API_KEY \
        --set-env-vars CENSUS_API_KEY=$CENSUS_API_KEY \
        --set-env-vars BEA_API_KEY=$BEA_API_KEY \
        --set-env-vars APIFY_API_KEY=$APIFY_API_KEY \
        --set-env-vars "^|^RUST_LOG=$RUST_LOG_VALUE" \
        --set-env-vars LOG_FORMAT=json

