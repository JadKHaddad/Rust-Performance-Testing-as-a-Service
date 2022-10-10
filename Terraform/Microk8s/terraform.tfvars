k8s_config_path   = "/var/snap/microk8s/current/credentials/client.config"
namespace         = "performance-testing"
pv_local_path     = "/kubernetes/performance-testing/Performance-Testing-Data"
worker_count      = 3
registry          = "localhost:32000"
image_pull_policy = "Always"
