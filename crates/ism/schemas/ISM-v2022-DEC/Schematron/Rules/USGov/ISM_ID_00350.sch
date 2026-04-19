<?xml version="1.0" encoding="UTF-8"?>
<?ICEA pattern?>
<?schematron-phases phaseids="BANNER STRUCTURECHECK"?>
<!-- Notices - Distribution Notice: 
           This document has been approved for Public Release and is available for use without restriction.
       -->
<sch:pattern xmlns:sch="http://purl.oclc.org/dsdl/schematron" id="ISM-ID-00350">
   <sch:p xmlns:ism="urn:us:gov:ic:ism" ism:classification="U" ism:ownerProducer="USA" class="ruleText">
      [ISM-ID-00350][Error] Exclusive Distribution information (i.e. @ism:nonICmarkings of the
      resource node contains [XD]) requires XD profile NTK metadata.
   </sch:p>
   <sch:p xmlns:ism="urn:us:gov:ic:ism" ism:classification="U" ism:ownerProducer="USA" class="codeDesc">
      If the document is an ISM_USGOV_RESOURCE and the resource nodes's @ism:nonICmarkings
      attribute contains [XD], the document must have XD profile NTK metadata. That is, there must be an NTK assertion
      with an ntk:AccessPolicy value of ‘urn:us:gov:ic:aces:ntk:xd’.
   </sch:p>
   <sch:rule id="ISM-ID-00350-R1" context="*[$ISM_USGOV_RESOURCE and generate-id(.) = generate-id($ISM_RESOURCE_ELEMENT) and util:containsAnyOfTheTokens(@ism:nonICmarkings, ('XD'))]">
      <sch:assert test="/*//ntk:AccessPolicy[.='urn:us:gov:ic:aces:ntk:xd']" flag="error" role="error">
         [ISM-ID-00350][Error] Exclusive Distribution information (i.e. @ism:nonICmarkings of the
         resource node contains [XD]) requires XD profile NTK metadata.
      </sch:assert>
   </sch:rule>
</sch:pattern>