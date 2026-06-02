FROM node:20-alpine
WORKDIR /oasis
COPY dist/oasis-kernel.mjs .
CMD ["node", "--input-type=module", "-e", "\
import { OasisKernel } from './oasis-kernel.mjs';\
const k = new OasisKernel({ dim: 32, tenantId: process.env.TENANT || 'default' });\
k.addAgent('agent-0', 'RUNNING', 'HARD_RT');\
console.log('OASIS kernel started. Tenant:', k.config.tenantId);\
setInterval(() => { k.tick(); if (k.getTickCount() % 100 === 0) console.log('tick', k.getTickCount(), 'alive:', k.isAlive()); }, 3);\
"]
