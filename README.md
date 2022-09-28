# Performace Testing as a Service
Built to run on Kubernetes, this service is designed to test the performance of given applications using locust

## Deployment using MicroK8s

* Use the project's root directory as your working directory

* Enable the necessary MicroK8s oddons
```sh
microk8s enable dns registry ingress metallb
```
* MicroK8s' default local registry's port is, ```32000```

* Build the helper images
```sh
docker build -t builder:latest -f Dockerfiles/Dockerfile.builder .
```
```sh
docker build -t runner:latest -f Dockerfiles/Dockerfile.runner .
```
* Build the images
```sh
docker build -t localhost:32000/master-release:latest -f Dockerfiles/Dockerfile.master-release .
```
```sh
docker build -t localhost:32000/worker-release:latest -f Dockerfiles/Dockerfile.worker-release .
```
```sh
docker build -t localhost:32000/frontend:latest -f Dockerfiles/Dockerfile.frontend .
```
* Or use docker compose to build the images
```sh
docker-compose -f Dockerfiles/Docker-compose.yaml build
```
* Push the images to the local registry
```sh
docker push localhost:32000/master-release:latest
docker push localhost:32000/worker-release:latest
docker push localhost:32000/frontend:latest
```
* Create the namespace
```sh
kubectl create namespace performance-testing
```
* Make things easier by setting kubectl's default namespace
```sh
kubectl config set-context --current --namespace=performance-testing
```
* Apply kubernetes yaml files
```sh
kubectl apply -f Kubernetes/pv-volume.yaml
kubectl apply -f Kubernetes/pv-claim.yaml
kubectl apply -f Kubernetes/configmap.yaml
kubectl apply -f Kubernetes/redis.yaml
kubectl apply -f Kubernetes/frontend.yaml
kubectl apply -f Kubernetes/master.yaml
kubectl apply -f Kubernetes/worker-1.yaml
kubectl apply -f Kubernetes/worker-2.yaml
kubectl apply -f Kubernetes/worker-loadbalancer.yaml
kubectl apply -f Kubernetes/ingress.yaml
```
* Add more workers by using the WorkerCreator
```sh
python3 Kubernetes/WorkerCreator/app.py 3

kubectl apply -f Kubernetes/worker-3.yaml
```
* Depending on your use case and kubernetes distribution, you may edit ```Kubernetes/pv-volume.yaml``` and ```Kubernetes/ingress.yaml```

* MicroK8s's default ingress' port is, ```80```

* After building new images, reapply the deployments
```sh
# kubectl rollout restart -n <namespace> deployment <deployment>

kubectl rollout restart deployment -n performance-testing frontend-deployment
kubectl rollout restart deployment -n performance-testing master-deployment
kubectl rollout restart deployment -n performance-testing worker-1-deployment
kubectl rollout restart deployment -n performance-testing worker-2-deployment

...
```
* ```Master``` and ```Worker``` images are built to run as non root users
* Local persistant volumes do not support mounting a directory with non root permissions
* If you wish to use another persistant volume type, please make sure to edit the ```securityContext``` configurations in ```Kubernetes/master.yaml```, ```Kubernetes/worker-<worerk_num>.yaml``` and ```Kubernetes/WorkerCreator/template.yaml```
```yaml
...
spec:
  ...
  template:
    ...
    spec:
      securityContext:
        runAsUser: 101
        runAsGroup: 101
        fsGroup: 101
    ...
...
```

## Architecture
![architecture](https://github.com/JadKHaddad/Rust-Performance-Testing-as-a-Service/blob/main/Assets/architecture.png?raw=true)

## Run with Docker-Compose

```sh
docker-compose -f Dockerfiles/Docker-compose.yaml up
```
* Entrypoint is ```host:8000```

## Contributors
* Jad K. Haddad <jadkhaddad@gmail.com>

## License & copyright
© 2022 Jad K. Haddad
Licensed under the [MIT License](LICENSE)

## Todo
* Locust does not stop if host is not valid
* Stop test before download, or create new zip on every request
* Other features..

