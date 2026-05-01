workspace {

    model {
        customer = person "Personal Banking Customer" "A customer of the bank with personal accounts"
        backoffice = person "Back Office Staff" "Administration and support staff"

        bankingSystem = softwareSystem "Internet Banking System" "Allows customers to view account info and make payments" {
            singlePageApp = container "Single-Page Application" "Provides banking functionality via browser" "JavaScript/Angular"
            mobileApp = container "Mobile App" "Provides banking functionality via mobile" "Flutter"
            apiApplication = container "API Application" "Provides banking functionality via JSON/HTTPS API" "Java/Spring Boot" {
                signinController = component "Sign In Controller" "Allows users to sign in" "Spring MVC Controller"
                accountsController = component "Accounts Controller" "Provides account information" "Spring MVC Controller"
                transferController = component "Transfer Controller" "Handles money transfers" "Spring MVC Controller"
                securityComponent = component "Security Component" "Authentication and authorization" "Spring Security"
                accountRepo = component "Account Repository" "Data access for accounts" "Spring Data JPA"
                transferRepo = component "Transfer Repository" "Data access for transfers" "Spring Data JPA"
            }
            database = container "Database" "Stores user, account, and transaction data" "Oracle 19c"
        }

        mainframe = softwareSystem "Mainframe Banking System" "Stores core banking records" "External"
        emailSystem = softwareSystem "E-Mail System" "SendGrid" "External"

        customer -> bankingSystem "Views account balances and makes payments"
        bankingSystem -> mainframe "Gets account info, makes payments"
        bankingSystem -> emailSystem "Sends e-mails"
        emailSystem -> customer "Sends e-mails to"
        backoffice -> mainframe "Uses"

        customer -> singlePageApp "Views account balances and makes payments" "HTTPS"
        customer -> mobileApp "Views account balances and makes payments" "HTTPS"
        singlePageApp -> apiApplication "Makes API calls" "JSON/HTTPS"
        mobileApp -> apiApplication "Makes API calls" "JSON/HTTPS"
        apiApplication -> database "Reads from and writes to" "JDBC"
        apiApplication -> mainframe "Gets account info, makes payments" "XML/HTTPS"
        apiApplication -> emailSystem "Sends e-mails" "SMTP"

        signinController -> securityComponent "Uses"
        accountsController -> accountRepo "Uses"
        accountsController -> mainframe "Gets account info" "XML/HTTPS"
        transferController -> transferRepo "Uses"
        transferController -> mainframe "Makes payments" "XML/HTTPS"
        transferController -> emailSystem "Sends confirmation" "SMTP"
        accountRepo -> database "Reads/writes" "JDBC"
        transferRepo -> database "Reads/writes" "JDBC"
    }

    views {
        systemContext bankingSystem "SystemContext" {
            include *
            autoLayout tb
        }

        container bankingSystem "Containers" {
            include *
            autoLayout tb
        }

        component apiApplication "Components" {
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
