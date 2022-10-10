locals {
  root_path_tmp = "/${replace(abspath(path.root), ":", "")}"
  root_path     = replace(local.root_path_tmp, "////", "/")
  is_linux      = length(regexall("/home/", lower(abspath(path.root)))) > 0
  ports = {
    redis = {
      service = {
        port        = 6379
        target_port = 6379
      }
      container = {
        container_port = 6379
      }
    }
    master = {
      service = {
        port        = 3000
        target_port = 3000
      }
      container = {
        container_port = 3000
      }
    }
    workers = {
      service = {
        port        = 5000
        target_port = 5000
      }
      loadbalancer = {
        port        = 5000
        target_port = 5000
      }
      container = {
        container_port = 5000
      }
    }
    frontend = {
      service = {
        port        = 80
        target_port = 80
      }
      container = {
        container_port = 80
      }
    }
  }
  consts = {
    redis_host  = "REDIS_HOST"
    master_ip   = "MASTER_IP"
    worker_name = "WORKER_NAME"
  }
  paths = {
    container = {
      mount_path = "/home/app/Backend/Performance-Testing-Data"
    },
    image = { 
      context_path        = "../../"
      dockerfiles_path    = "Dockerfiles"
    }
  }
  services = {
    redis = {
      name = "redis-service"
    },
    master = {
      name = "master-service"
    },
    frontend = {
      name = "frontend-service"
    },
    workers = {
      loadbalancer = {
        name = "worker-loadbalancer"
      }
    }
  }
  deployments = {
    redis = {
      name = "redis-deployment"
    },
    master = {
      name = "master-deployment"
    },
    frontend = {
      name = "frontend-deployment"
    },
  }
  session_affinity = "None"
}