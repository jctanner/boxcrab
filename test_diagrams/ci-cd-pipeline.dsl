workspace {

    model {
        developer = person "Developer" "Writes and reviews code"
        releaseManager = person "Release Manager" "Approves production deploys"

        cicd = softwareSystem "CI/CD Platform" "Automated build, test, and deployment pipeline" {
            sourceControl = container "Source Control" "Git repositories" "GitHub"
            ciServer = container "CI Server" "Runs builds and tests" "GitHub Actions"
            artifactRegistry = container "Artifact Registry" "Stores built container images" "GHCR"
            securityScanner = container "Security Scanner" "SAST, DAST, dependency scanning" "Snyk"
            stagingCluster = container "Staging Cluster" "Pre-production environment" "Kubernetes"
            productionCluster = container "Production Cluster" "Live environment" "Kubernetes"
            configStore = container "Config Store" "Environment configuration and secrets" "Vault"
            monitoring = container "Monitoring Stack" "Metrics, logs, and alerts" "Prometheus/Grafana"
            gitopsController = container "GitOps Controller" "Syncs desired state to clusters" "ArgoCD"
        }

        slackNotify = softwareSystem "Slack" "Team notifications" "External"
        pagerDuty = softwareSystem "PagerDuty" "On-call alerting" "External"

        developer -> cicd "Pushes code, reviews PRs"
        releaseManager -> cicd "Approves deployments"
        cicd -> slackNotify "Sends build notifications"
        cicd -> pagerDuty "Sends production alerts"

        developer -> sourceControl "Pushes code" "Git/SSH"
        releaseManager -> gitopsController "Approves promotion" "HTTPS"
        sourceControl -> ciServer "Triggers builds" "Webhook"
        ciServer -> securityScanner "Runs security checks" "API"
        ciServer -> artifactRegistry "Pushes images" "OCI"
        ciServer -> slackNotify "Notifies on build status" "Webhook"
        artifactRegistry -> gitopsController "Image available" "Webhook"
        gitopsController -> stagingCluster "Deploys to staging" "Kubernetes API"
        gitopsController -> productionCluster "Deploys to production" "Kubernetes API"
        gitopsController -> configStore "Reads secrets" "HTTPS"
        stagingCluster -> monitoring "Exports metrics" "Prometheus"
        productionCluster -> monitoring "Exports metrics" "Prometheus"
        monitoring -> pagerDuty "Fires alerts" "Webhook"
        monitoring -> slackNotify "Sends dashboards" "Webhook"
    }

    views {
        systemContext cicd "SystemContext" {
            include *
            autoLayout tb
        }

        container cicd "Containers" {
            include *
            autoLayout lr
        }

        styles {
            element "Person" {
                background #4A148C
                color #ffffff
                shape Rounded
            }
            element "Software System" {
                background #7B1FA2
                color #ffffff
            }
            element "External" {
                background #9E9E9E
                color #ffffff
            }
            element "Container" {
                background #CE93D8
                color #000000
            }
        }
    }
}
