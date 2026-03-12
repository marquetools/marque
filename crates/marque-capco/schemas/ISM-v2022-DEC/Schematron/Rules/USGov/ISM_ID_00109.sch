<?xml version="1.0" encoding="UTF-8"?>
<?ICEA pattern?>
<?schematron-phases phaseids="ROLLDOWN VALUECHECK"?>
<!-- Notices - Distribution Notice: 
           This document has been approved for Public Release and is available for use without restriction.
       -->
<sch:pattern xmlns:sch="http://purl.oclc.org/dsdl/schematron" id="ISM-ID-00109" is-a="NonCompilationDocumentRollup">
  <sch:p xmlns:ism="urn:us:gov:ic:ism" ism:classification="U" ism:ownerProducer="USA" class="ruleText">
    [ISM-ID-00109][Error] If ISM_USGOV_RESOURCE and attribute @ism:classification
    of ISM_RESOURCE_ELEMENT has a value of [S] and attribute @ism:compilationReason does not have a value then at least
    one element meeting ISM_CONTRIBUTES in the document must have a @ism:classification attribute of [S].
    
    Human Readable: USA S documents not using compilation must have S data.
  </sch:p>
  <sch:p xmlns:ism="urn:us:gov:ic:ism" ism:classification="U" ism:ownerProducer="USA" class="codeDesc">
    This rule uses an abstract pattern to consolidate logic. If the document
    is an ISM_USGOV_RESOURCE and attribute $attrLocalName of ISM_RESOURCE_ELEMENT 
    has a value of [$value] and attribute @ism:compilationReason does not have a 
    value, then this rule ensures that at least one element meeting ISM_CONTRIBUTES 
    specifies attribute $attrLocalName with a value of [$value].
  </sch:p>
  <sch:param name="attrLocalName" value="classification"/>
  <sch:param name="value" value="S"/>
  <sch:param name="errorMessage" value="'[ISM-ID-00109][Error] USA S documents not using compilation must have S data.'"/>
</sch:pattern>