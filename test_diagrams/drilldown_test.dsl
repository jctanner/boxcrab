workspace {

    model {
        user = person "End User" "A customer using the platform"
        admin = person "Admin" "Platform administrator"

        platform = softwareSystem "E-Commerce Platform" "Online shopping platform" {
            webApp = container "Web Application" "React SPA served to browsers" "React/TypeScript" {
                productCatalog = component "Product Catalog" "Displays products and search results" "React Component"
                shoppingCart = component "Shopping Cart" "Manages cart state and checkout flow" "React Component"
                authModule = component "Auth Module" "Handles login, signup, OAuth" "React Component"
            }
            apiGateway = container "API Gateway" "Routes requests, rate limiting, auth" "Kong" {
                rateLimiter = component "Rate Limiter" "Token bucket rate limiting" "Kong Plugin"
                authMiddleware = component "Auth Middleware" "JWT validation and RBAC" "Kong Plugin"
                routeManager = component "Route Manager" "Dynamic route configuration" "Kong Plugin"
            }
            orderService = container "Order Service" "Handles order lifecycle" "Go" {
                orderApi = component "Order API" "REST endpoints for orders" "Go HTTP Handler"
                orderProcessor = component "Order Processor" "Processes and validates orders" "Go Service"
                paymentIntegration = component "Payment Integration" "Stripe/PayPal integration" "Go Client"
            }
            productService = container "Product Service" "Product catalog and inventory" "Python"
            userService = container "User Service" "User accounts and profiles" "Go"
            notificationService = container "Notification Service" "Email, SMS, push notifications" "Node.js"
            database = container "Database" "Primary data store" "PostgreSQL"
            cache = container "Cache" "Session and query cache" "Redis"
            messageQueue = container "Message Queue" "Async event processing" "RabbitMQ"
        }

        paymentProvider = softwareSystem "Payment Provider" "Stripe payment processing" "External"
        emailService = softwareSystem "Email Service" "SendGrid email delivery" "External"
        cdnProvider = softwareSystem "CDN" "CloudFront content delivery" "External"

        user -> platform "Browses and purchases products" "HTTPS"
        admin -> platform "Manages products and orders" "HTTPS"
        platform -> paymentProvider "Processes payments" "HTTPS/API"
        platform -> emailService "Sends transactional emails" "SMTP"
        platform -> cdnProvider "Serves static assets" "HTTPS"

        user -> webApp "Uses" "HTTPS"
        admin -> webApp "Manages" "HTTPS"
        webApp -> apiGateway "Makes API calls" "HTTPS/JSON"
        apiGateway -> orderService "Routes order requests" "gRPC"
        apiGateway -> productService "Routes product requests" "gRPC"
        apiGateway -> userService "Routes user requests" "gRPC"
        orderService -> database "Reads/writes orders" "SQL"
        orderService -> messageQueue "Publishes order events" "AMQP"
        orderService -> paymentProvider "Processes payments" "HTTPS"
        productService -> database "Reads/writes products" "SQL"
        productService -> cache "Caches product data" "Redis Protocol"
        userService -> database "Reads/writes users" "SQL"
        userService -> cache "Caches sessions" "Redis Protocol"
        notificationService -> emailService "Sends emails" "SMTP"
        messageQueue -> notificationService "Delivers events" "AMQP"

        productCatalog -> apiGateway "Fetches products" "HTTPS/JSON"
        shoppingCart -> apiGateway "Submits orders" "HTTPS/JSON"
        authModule -> apiGateway "Authenticates" "HTTPS/JSON"

        rateLimiter -> authMiddleware "Passes allowed requests"
        authMiddleware -> routeManager "Passes authenticated requests"

        orderApi -> orderProcessor "Delegates processing"
        orderProcessor -> paymentIntegration "Requests payment"
        paymentIntegration -> paymentProvider "Charges card" "HTTPS"
    }

    views {
        systemContext platform "SystemContext" {
            include *
            autoLayout lr
        }

        container platform "Containers" {
            include *
            autoLayout lr
        }

        component webApp "WebAppComponents" {
            include *
            autoLayout lr
        }

        component apiGateway "ApiGatewayComponents" {
            include *
            autoLayout lr
        }

        component orderService "OrderServiceComponents" {
            include *
            autoLayout lr
        }

        styles {
            element "Person" {
                background #08427B
                color #ffffff
                shape Rounded
            }
            element "Software System" {
                background #1168BD
                color #ffffff
            }
            element "External" {
                background #999999
                color #ffffff
            }
            element "Container" {
                background #438DD5
                color #ffffff
            }
            element "Component" {
                background #85BBF0
                color #000000
            }
        }
    }
}
