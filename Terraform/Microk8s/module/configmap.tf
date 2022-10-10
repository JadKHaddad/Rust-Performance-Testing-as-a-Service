resource "kubernetes_config_map" "configmap" {
  metadata {
    name      = "configmap"
    namespace = kubernetes_namespace.main_namespace.metadata.0.name
  }
  data = {
    redis_host = local.services.redis.name
    master_ip  = "${local.services.master.name}:${local.ports.master.service.port}"
  }
}