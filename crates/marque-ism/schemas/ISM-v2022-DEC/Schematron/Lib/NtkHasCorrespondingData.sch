<?xml version="1.0" encoding="UTF-8"?>
<!-- Notices - Distribution Notice: 
           This document has been approved for Public Release and is available for use without restriction.
       -->
<sch:pattern xmlns:sch="http://purl.oclc.org/dsdl/schematron" abstract="true" id="NtkHasCorrespondingData">
   <sch:p xmlns:ism="urn:us:gov:ic:ism" ism:classification="U" ism:ownerProducer="USA" class="codeDesc">
      Abstract template to validate that for an $ISM_USGOV_RESOURCE, a given token ($dataType)
      exists in a particular attribute of at least one of 
      (a) a portion that contributes to roll-up or 
      (b) the banner, given the existence of an ntk:AccessProfile that has an ntk:AccessPolicy value that starts with a given string
      ($uriPrefix).
   </sch:p>
   <sch:p xmlns:ism="urn:us:gov:ic:ism" ism:classification="U" ism:ownerProducer="USA" class="codeDesc">
      Expected parameters: $ruleId, $policyName, $uriPrefix, $attr, $dataType, $dataTokenList, and
      $bannerTokenList
   </sch:p>
   <sch:rule id="NtkHasCorrespondingData-R1" context="ntk:Access//ntk:AccessProfile[ntk:AccessPolicy[starts-with(., $uriPrefix)] and $ISM_USGOV_RESOURCE]">
      <sch:assert test="index-of($dataTokenList, $dataType)&gt;0 or index-of($bannerTokenList, $dataType)&gt;0" flag="error" role="error">
         [<sch:value-of select="$ruleId"/>][error] <sch:value-of select="$policyName"/> NTK metadata
         requires that <sch:value-of select="$attr"/> contain <sch:value-of select="$dataType"/> in at least one of (a)
         a portion that contributes to roll-up or (b) the banner.</sch:assert>
   </sch:rule>
</sch:pattern>