export const slugOf = (name: string) => name.replace(/^tailored\//, '')
export const cvTitle = (name: string) => (name === 'main' ? 'Main CV' : slugOf(name))
