import { createRouter, createWebHistory } from 'vue-router'
import Home from '../views/Home.vue'

const routes = [
  {
    path: '/',
    name: 'Home',
    component: Home
  },
  {
    path: '/control',
    name: 'Control',
    component: () => import('../views/Control.vue')

  },
  {
    path: '/project/:pid/:id',
    name: 'Script',
    component: () => import('../views/project/Script.vue'),
    props: true
  },
  {
    path: '/check/:script',
    name: 'Check',
    component: () => import('../views/Check.vue'),
    props: true
  },
  //404
  {
    path: '/:catchAll(.*)',
    name: 'NotFound',
    component: () => import('../views/NotFound.vue')
  },
]

const router = createRouter({
  history: createWebHistory(process.env.BASE_URL),
  routes
})

export default router
