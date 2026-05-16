import React from 'react'

interface WorkflowBarProps {
  activeStep: 1 | 2 | 3
  mode?: 'workflow' | 'audit'
}

const STEPS = [
  { num: 1, label: 'Scan' },
  { num: 2, label: 'Review' },
  { num: 3, label: 'Clean' },
]

export default function WorkflowBar({ activeStep, mode = 'workflow' }: WorkflowBarProps) {
  if (mode === 'audit') {
    return (
      <div className="workflow-bar">
        <span
          style={{
            fontFamily: 'var(--font-mono)',
            fontSize: '0.65rem',
            letterSpacing: '0.1em',
            textTransform: 'uppercase',
            color: 'var(--text-muted-light)',
            marginLeft: 'auto',
          }}
        >
          Audit Log
        </span>
      </div>
    )
  }

  return (
    <div className="workflow-bar">
      {STEPS.map((step, i) => {
        const state = step.num < activeStep ? 'done' : step.num === activeStep ? 'active' : 'pending'
        return (
          <React.Fragment key={step.num}>
            <div className={`wf-step wf-step-${state}`}>
              <span className="wf-step-num">{step.num}</span>
              <span>{step.label}</span>
            </div>
            {i < STEPS.length - 1 && (
              <span className="wf-sep">→</span>
            )}
          </React.Fragment>
        )
      })}
    </div>
  )
}