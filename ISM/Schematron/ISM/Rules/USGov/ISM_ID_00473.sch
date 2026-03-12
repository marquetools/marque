<?xml version="1.0" encoding="UTF-8"?>
<?ICEA pattern?>
<?schematron-phases phaseids="BANNER PORTION VALUECHECK"?>
<sch:pattern xmlns:sch="http://purl.oclc.org/dsdl/schematron" id="ISM-ID-00473">
   <sch:p xmlns:ism="urn:us:gov:ic:ism" ism:classification="U" ism:ownerProducer="USA" class="ruleText">
      [ISM-ID-00473][Error] If ISM_USGOV_RESOURCE, PROPIN information (i.e. @ism:disseminationControls of the resource node 
      contains [PR]) requires explicit Foreign Disclosure &amp; Release (FD&amp;R) markings ([REL], [RELIDO], [NF], [DISPLAYONLY] 
      or [EYES]).
   </sch:p>
   <sch:p xmlns:ism="urn:us:gov:ic:ism" ism:classification="U" ism:ownerProducer="USA" class="codeDesc">
      If the document is an ISM_USGOV_RESOURCE, then any element that contains 
      @ism:disseminationControls attribute contains [PR], the document must have one of: [REL], [RELIDO], [NF], [DISPLAYONLY] or [EYES].
   </sch:p>
   <sch:rule id="ISM-ID-00473-R1" context="*[$ISM_USGOV_RESOURCE and (util:containsAnyOfTheTokens(@ism:disseminationControls, ('PR')))]">
      <sch:assert test="util:containsAnyOfTheTokens(@ism:disseminationControls, ('REL','RELIDO','NF','DISPLAYONLY','EYES'))" flag="error" role="error">
         [ISM-ID-00473][Error]  If ISM_USGOV_RESOURCE, PROPIN information (i.e. @ism:disseminationControls of the resource node 
         contains [PR]) requires explicit Foreign Disclosure &amp; Release (FD&amp;R) markings ([REL], [RELIDO], [NF], [DISPLAYONLY] 
         or [EYES]).
      </sch:assert>
   </sch:rule>
</sch:pattern>
