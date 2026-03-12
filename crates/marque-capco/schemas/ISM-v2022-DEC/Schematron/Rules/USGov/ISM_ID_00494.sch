<?xml version="1.0" encoding="UTF-8"?>
<?ICEA pattern?>
<?schematron-phases phaseids="BANNER STRUCTURECHECK"?>
<!-- Notices - Distribution Notice: 
           This document has been approved for Public Release and is available for use without restriction.
       -->
<sch:pattern xmlns:sch="http://purl.oclc.org/dsdl/schematron" id="ISM-ID-00494">
   <sch:p xmlns:ism="urn:us:gov:ic:ism" ism:classification="U" ism:ownerProducer="USA" class="ruleText">
      [ISM-ID-00494][Error] If ISM_USCUI_RESOURCE or ISM_USCUIONLY_RESOURCE, then if
      the document contains a PROPIN CUI Category marking (either Basic or Specified), then the
      document MUST have PROPIN_NTK metadata.
      
      Human Readable: PROPIN CUI information (either @ism:cuiBasic or
      @ism:cuiSpecified contains 'PROPIN') requires PROPIN NTK metadata.
   </sch:p>
   <sch:p xmlns:ism="urn:us:gov:ic:ism" ism:classification="U" ism:ownerProducer="USA" class="codeDesc">
      If the document is an ISM_USCUI_RESOURCE or ISM_USCUIONLY_RESOURCE, and the
      resource node's @ism:cuiBasic or @ism:cuiSpecified attribute contains [PROPIN], then the document must
      have PROPIN NTK profile metadata. That is, there must be an NTK assertion with an
      ntk:AccessPolicy value that starts with ‘urn:us:gov:ic:aces:ntk:propin:’.
   </sch:p>
   <sch:rule id="ISM-ID-00494-R1" context="*[($ISM_USCUI_RESOURCE or $ISM_USCUIONLY_RESOURCE) and generate-id(.) = generate-id($ISM_RESOURCE_ELEMENT) and (util:containsAnyOfTheTokens(@ism:cuiBasic, ('PROPIN')) or util:containsAnyOfTheTokens(@ism:cuiSpecified, ('PROPIN')))]">
      <sch:assert test="/*//ntk:AccessPolicy[starts-with(., 'urn:us:gov:ic:aces:ntk:propin:')]" flag="error" role="error">
         [ISM-ID-00494][Error] If ISM_USCUI_RESOURCE or ISM_USCUIONLY_RESOURCE, then if
         the document contains a PROPIN CUI Category marking (either Basic or Specified), then the
         document MUST have PROPIN_NTK metadata.
         
         Human Readable: PROPIN CUI information (either @ism:cuiBasic or
         @ism:cuiSpecified contains 'PROPIN') requires PROPIN NTK metadata.
      </sch:assert>
   </sch:rule>
</sch:pattern>
