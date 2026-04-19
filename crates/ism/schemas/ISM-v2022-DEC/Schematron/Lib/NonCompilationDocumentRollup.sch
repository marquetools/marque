<?xml version="1.0" encoding="UTF-8"?>
<!-- Notices - Distribution Notice: 
           This document has been approved for Public Release and is available for use without restriction.
       -->
<sch:pattern xmlns:sch="http://purl.oclc.org/dsdl/schematron" abstract="true" id="NonCompilationDocumentRollup">
  <sch:p xmlns:ism="urn:us:gov:ic:ism" ism:classification="U" ism:ownerProducer="USA" class="codeDesc">
    If ISM_USGOV_RESOURCE and attribute $attrLocalName of ISM_RESOURCE_ELEMENT 
    has a value of [$value] and attribute @ism:compilationReason does not have a 
    value, then this rule ensures that at least one element meeting ISM_CONTRIBUTES 
    specifies attribute $attrLocalName with a value of [$value].
  </sch:p>
  <sch:rule id="NonCompilationDocumentRollup-R1" context="*[$ISM_USGOV_RESOURCE and generate-id(.) = generate-id($ISM_RESOURCE_ELEMENT) and util:containsAnyOfTheTokens(@ism:$attrLocalName, ('$value')) and string-length(normalize-space(@ism:compilationReason)) = 0]">
      <sch:assert test="some $ele in $partTags satisfies util:containsAnyOfTheTokens($ele/@ism:$attrLocalName, ('$value'))" flag="error" role="error">
			      <sch:value-of select="$errorMessage"/>
      </sch:assert>
	  </sch:rule>
</sch:pattern>