<?xml version="1.0" encoding="UTF-8"?>
<?ICEA pattern?>
<?schematron-phases phaseids="BANNER STRUCTURECHECK"?>
<!-- Notices - Distribution Notice: 
           This document has been approved for Public Release and is available for use without restriction.
       -->
<sch:pattern xmlns:sch="http://purl.oclc.org/dsdl/schematron" id="ISM-ID-00349">
   <sch:p xmlns:ism="urn:us:gov:ic:ism" ism:classification="U" ism:ownerProducer="USA" class="ruleText">
      [ISM-ID-00349][Error] If ISM_USGOV_RESOURCE, PROPIN information (i.e. @ism:disseminationControls of the resource
      node contains [PR]) requires PROPIN NTK metadata.
   </sch:p>
   <sch:p xmlns:ism="urn:us:gov:ic:ism" ism:classification="U" ism:ownerProducer="USA" class="codeDesc">
      If the document is an ISM_USGOV_RESOURCE and the resource node's @ism:disseminationControls
      attribute contains [PR], the document must have PROPIN profile NTK metadata. That is, there must be an NTK
      assertion with an ntk:AccessPolicy value that starts with ‘urn:us:gov:ic:aces:ntk:propin:’.
   </sch:p>
   <sch:rule id="ISM-ID-00349-R1" context="*[$ISM_USGOV_RESOURCE and generate-id(.) = generate-id($ISM_RESOURCE_ELEMENT) and util:containsAnyOfTheTokens(@ism:disseminationControls, ('PR'))]">
      <sch:assert test="/*//ntk:AccessPolicy[starts-with(.,'urn:us:gov:ic:aces:ntk:propin:')]" flag="error" role="error">
         [ISM-ID-00349][Error] If ISM_USGOV_RESOURCE, PROPIN information (i.e. @ism:disseminationControls of the resource
         node contains [PR]) requires PROPIN NTK metadata.
      </sch:assert>
   </sch:rule>
</sch:pattern>