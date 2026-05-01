workspace {

    model {
        mobileUser = person "Mobile User" "Uses the app on their phone"
        webUser = person "Web User" "Uses the app in a browser"

        platform = softwareSystem "Ride Sharing Platform" "Connects riders with drivers" {
            riderApp = container "Rider App" "iOS and Android app for riders" "React Native"
            driverApp = container "Driver App" "iOS and Android app for drivers" "React Native"
            webDashboard = container "Web Dashboard" "Admin and analytics dashboard" "React"
            apiGateway = container "API Gateway" "Authentication, routing, rate limiting" "Envoy"
            rideService = container "Ride Service" "Manages ride lifecycle" "Go"
            matchingService = container "Matching Service" "Matches riders to nearby drivers" "Rust"
            pricingService = container "Pricing Service" "Dynamic surge pricing calculations" "Python"
            locationService = container "Location Service" "Real-time GPS tracking" "Go"
            paymentService = container "Payment Service" "Handles billing and payouts" "Java"
            notifyService = container "Notification Service" "Push notifications and SMS" "Node.js"
            rideDb = container "Ride Database" "Ride history and state" "PostgreSQL"
            locationDb = container "Location Store" "Real-time driver positions" "Redis"
            eventBus = container "Event Bus" "Async messaging between services" "Apache Kafka"
            objectStore = container "Object Storage" "Trip receipts and documents" "MinIO"
        }

        paymentGateway = softwareSystem "Payment Gateway" "Stripe" "External"
        mapProvider = softwareSystem "Map Provider" "Google Maps Platform" "External"
        smsProvider = softwareSystem "SMS Provider" "Twilio" "External"

        mobileUser -> platform "Requests rides"
        webUser -> platform "Monitors fleet and analytics"
        platform -> paymentGateway "Processes payments"
        platform -> mapProvider "Gets routes and ETAs"
        platform -> smsProvider "Sends SMS notifications"

        mobileUser -> riderApp "Requests rides"
        mobileUser -> driverApp "Accepts rides"
        webUser -> webDashboard "Views analytics"
        riderApp -> apiGateway "API calls" "HTTPS"
        driverApp -> apiGateway "API calls" "HTTPS"
        webDashboard -> apiGateway "API calls" "HTTPS"
        apiGateway -> rideService "Routes ride requests" "gRPC"
        apiGateway -> locationService "Routes location updates" "gRPC"
        apiGateway -> paymentService "Routes payment requests" "gRPC"
        rideService -> matchingService "Finds available drivers" "gRPC"
        rideService -> pricingService "Gets fare estimate" "gRPC"
        rideService -> rideDb "Stores ride data" "SQL"
        rideService -> eventBus "Publishes ride events" "Kafka"
        matchingService -> locationDb "Queries driver positions" "Redis"
        locationService -> locationDb "Updates driver positions" "Redis"
        locationService -> mapProvider "Calculates routes" "HTTPS"
        paymentService -> paymentGateway "Charges riders, pays drivers" "HTTPS"
        paymentService -> rideDb "Reads ride totals" "SQL"
        eventBus -> notifyService "Delivers ride events" "Kafka"
        notifyService -> smsProvider "Sends SMS" "HTTPS"
        rideService -> objectStore "Stores receipts" "S3 API"
    }

    views {
        systemContext platform "SystemContext" {
            include *
            autoLayout tb
        }

        container platform "Containers" {
            include *
            autoLayout lr
        }

        styles {
            element "Person" {
                background #2D882D
                color #ffffff
                shape Rounded
            }
            element "Software System" {
                background #55AA55
                color #ffffff
            }
            element "External" {
                background #888888
                color #ffffff
            }
            element "Container" {
                background #77CC77
                color #000000
            }
        }
    }
}
