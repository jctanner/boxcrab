workspace {

    model {
        dataScientist = person "Data Scientist" "Develops and trains ML models"
        mlEngineer = person "ML Engineer" "Deploys and monitors models in production"

        mlPlatform = softwareSystem "ML Platform" "End-to-end machine learning lifecycle" {
            notebook = container "Notebook Server" "Interactive development environment" "JupyterHub"
            featureStore = container "Feature Store" "Centralized feature definitions and serving" "Feast"
            trainingPipeline = container "Training Pipeline" "Orchestrates model training jobs" "Kubeflow Pipelines"
            modelRegistry = container "Model Registry" "Version control for trained models" "MLflow"
            servingLayer = container "Serving Layer" "Real-time model inference" "KServe"
            dataLake = container "Data Lake" "Raw and processed datasets" "S3/Parquet"
            metadataDb = container "Metadata DB" "Experiment tracking and lineage" "PostgreSQL"
            gpuCluster = container "GPU Cluster" "Training compute resources" "NVIDIA A100s/Kubernetes"
            monitoringDashboard = container "Monitoring" "Model performance and data drift" "Evidently AI"
        }

        dataWarehouse = softwareSystem "Data Warehouse" "Snowflake analytics" "External"
        labelingService = softwareSystem "Labeling Service" "Scale AI data labeling" "External"
        cloudStorage = softwareSystem "Cloud Storage" "AWS S3" "External"

        dataScientist -> mlPlatform "Develops and trains models"
        mlEngineer -> mlPlatform "Deploys and monitors models"
        mlPlatform -> dataWarehouse "Queries training data"
        mlPlatform -> labelingService "Submits labeling tasks"
        mlPlatform -> cloudStorage "Stores artifacts"

        dataScientist -> notebook "Writes experiments" "HTTPS"
        mlEngineer -> modelRegistry "Promotes models" "HTTPS"
        mlEngineer -> monitoringDashboard "Monitors production models" "HTTPS"
        notebook -> featureStore "Retrieves features" "gRPC"
        notebook -> dataLake "Reads datasets" "S3 API"
        notebook -> trainingPipeline "Submits training runs" "Kubernetes API"
        trainingPipeline -> gpuCluster "Schedules training jobs" "Kubernetes API"
        trainingPipeline -> featureStore "Reads training features" "gRPC"
        trainingPipeline -> modelRegistry "Registers trained models" "REST"
        trainingPipeline -> metadataDb "Logs experiments" "SQL"
        trainingPipeline -> dataLake "Reads training data" "S3 API"
        modelRegistry -> servingLayer "Deploys models" "Kubernetes API"
        servingLayer -> featureStore "Gets online features" "gRPC"
        servingLayer -> monitoringDashboard "Exports predictions" "HTTPS"
        featureStore -> dataWarehouse "Materializes features" "SQL"
        featureStore -> dataLake "Stores offline features" "S3 API"
        monitoringDashboard -> metadataDb "Reads experiment data" "SQL"
        dataLake -> cloudStorage "Backed by" "S3 API"
        trainingPipeline -> labelingService "Requests labels" "HTTPS"
    }

    views {
        systemContext mlPlatform "SystemContext" {
            include *
            autoLayout tb
        }

        container mlPlatform "Containers" {
            include *
            autoLayout tb
        }

        styles {
            element "Person" {
                background #B71C1C
                color #ffffff
                shape Rounded
            }
            element "Software System" {
                background #E53935
                color #ffffff
            }
            element "External" {
                background #757575
                color #ffffff
            }
            element "Container" {
                background #EF9A9A
                color #000000
            }
        }
    }
}
