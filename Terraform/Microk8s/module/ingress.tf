resource "kubernetes_ingress_v1" "ingress" {
  metadata {
    name = "ingress"
    annotations = {
      "nginx.ingress.kubernetes.io/rewrite-target" = "/$1"
      "kubernetes.io/ingress.class"                = "public"
    }
    namespace = kubernetes_namespace.main_namespace.metadata.0.name
  }
  spec {
    rule {
      http {
        path {
          path      = "/(explore.*)"
          path_type = "Prefix"
          backend {
            service {
              name = kubernetes_service.master_service.metadata.0.name
              port {
                number = kubernetes_service.master_service.spec.0.port.0.port
              }
            }
          }
        }
        path {
          path      = "/api/master/(.*)"
          path_type = "Prefix"
          backend {
            service {
              name = kubernetes_service.master_service.metadata.0.name
              port {
                number = kubernetes_service.master_service.spec.0.port.0.port
              }
            }
          }
        }
        path {
          path      = "/api/worker/(.*)"
          path_type = "Prefix"
          backend {
            service {
              name = kubernetes_service.worker_loadbalancer.metadata.0.name
              port {
                number = kubernetes_service.worker_loadbalancer.spec.0.port.0.port
              }
            }
          }
        }
        path {
          path      = "/(.*)"
          path_type = "Prefix"
          backend {
            service {
              name = kubernetes_service.frontend_service.metadata.0.name
              port {
                number = kubernetes_service.frontend_service.spec.0.port.0.port
              }
            }
          }
        }
      }
    }
  }
}