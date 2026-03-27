import { Component, type ReactNode } from 'react'

interface Props {
  children: ReactNode
  fallback?: ReactNode
  name?: string
}

interface State {
  error: Error | null
}

export class ErrorBoundary extends Component<Props, State> {
  state: State = { error: null }

  static getDerivedStateFromError(error: Error): State {
    return { error }
  }

  componentDidCatch(error: Error, info: React.ErrorInfo) {
    console.error(`[ErrorBoundary${this.props.name ? `:${this.props.name}` : ''}]`, error, info.componentStack)
  }

  render() {
    if (this.state.error) {
      if (this.props.fallback) return this.props.fallback
      return (
        <div style={{
          display: 'flex', alignItems: 'center', justifyContent: 'center',
          width: '100%', height: '100%', minHeight: 60,
          background: '#1a1a1a', color: '#e05560',
          fontFamily: 'monospace', fontSize: 11, padding: 12,
          flexDirection: 'column', gap: 6,
        }}>
          <span>{this.props.name ?? 'Component'} error</span>
          <span style={{ fontSize: 9, color: '#888', maxWidth: 300, overflow: 'hidden', textOverflow: 'ellipsis' }}>
            {this.state.error.message}
          </span>
          <button
            onClick={() => this.setState({ error: null })}
            style={{
              background: '#e0556022', border: '1px solid #e0556066', color: '#e05560',
              fontFamily: 'monospace', fontSize: 10, padding: '3px 10px',
              borderRadius: 3, cursor: 'pointer', marginTop: 4,
            }}
          >Retry</button>
        </div>
      )
    }
    return this.props.children
  }
}
